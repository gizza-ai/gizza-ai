//! gizza-ai/check-digit-validator core — validate (or compute) the check digit of a
//! code against every common check-digit scheme: Luhn/mod-10 (payment cards, IMEI,
//! NPI), the GS1 mod-10 family (EAN-8/13, UPC-A, GTIN-14, SSCC-18, ISBN-13), the
//! mod-11 family (ISBN-10, ISSN, VIN), ISIN, ABA routing numbers, and IBAN
//! (mod-97-10). With `scheme = auto` it reports which schemes the code could be and
//! which of them it actually passes.
//!
//! Pure: no dependencies, no I/O, no clock. Shared by the chat/CLI block and the
//! browser page.

/// Maximum number of codes accepted in one batch run.
pub const MAX_ITEMS: usize = 5000;

/// Characters allowed between/around a code's characters; they are stripped
/// before checking so `4539 1488 0343 6467` and `4539-1488-0343-6467` both work.
const SEPARATORS: &[char] = &[' ', '\t', '-', '.', '_', '/', ':'];

// ---------------------------------------------------------------------------
// Schemes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    Luhn,
    CreditCard,
    Imei,
    Npi,
    Isbn10,
    Isbn13,
    Issn,
    Ean8,
    Ean13,
    UpcA,
    Gtin14,
    Sscc,
    Iban,
    Routing,
    Isin,
    Vin,
}

/// Canonical enum values, in the order the descriptor advertises them.
pub const SCHEME_VALUES: [&str; 16] = [
    "luhn",
    "credit-card",
    "imei",
    "npi",
    "isbn10",
    "isbn13",
    "issn",
    "ean8",
    "ean13",
    "upc-a",
    "gtin14",
    "sscc",
    "iban",
    "routing",
    "isin",
    "vin",
];

impl Scheme {
    /// Canonical kebab-case value (what the descriptor/CLI/page use).
    pub fn value(self) -> &'static str {
        match self {
            Scheme::Luhn => "luhn",
            Scheme::CreditCard => "credit-card",
            Scheme::Imei => "imei",
            Scheme::Npi => "npi",
            Scheme::Isbn10 => "isbn10",
            Scheme::Isbn13 => "isbn13",
            Scheme::Issn => "issn",
            Scheme::Ean8 => "ean8",
            Scheme::Ean13 => "ean13",
            Scheme::UpcA => "upc-a",
            Scheme::Gtin14 => "gtin14",
            Scheme::Sscc => "sscc",
            Scheme::Iban => "iban",
            Scheme::Routing => "routing",
            Scheme::Isin => "isin",
            Scheme::Vin => "vin",
        }
    }

    /// Human-readable label used in the report.
    pub fn label(self) -> &'static str {
        match self {
            Scheme::Luhn => "Luhn (mod-10)",
            Scheme::CreditCard => "Credit card (Luhn mod-10)",
            Scheme::Imei => "IMEI (15-digit Luhn)",
            Scheme::Npi => "NPI (10-digit Luhn)",
            Scheme::Isbn10 => "ISBN-10 (mod-11)",
            Scheme::Isbn13 => "ISBN-13 (GS1 mod-10)",
            Scheme::Issn => "ISSN (mod-11)",
            Scheme::Ean8 => "EAN-8 / GTIN-8",
            Scheme::Ean13 => "EAN-13 / GTIN-13",
            Scheme::UpcA => "UPC-A / GTIN-12",
            Scheme::Gtin14 => "GTIN-14 / ITF-14",
            Scheme::Sscc => "SSCC-18",
            Scheme::Iban => "IBAN (mod-97-10)",
            Scheme::Routing => "ABA routing number",
            Scheme::Isin => "ISIN (Luhn on expanded letters)",
            Scheme::Vin => "VIN (mod-11 transliteration)",
        }
    }

    /// Parse a user-supplied scheme name. Accepts the canonical value plus the
    /// obvious spelling variants an LLM or a human might type.
    pub fn parse(s: &str) -> Option<Scheme> {
        let k: String = s
            .trim()
            .to_ascii_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();
        Some(match k.as_str() {
            "luhn" | "mod10" => Scheme::Luhn,
            "creditcard" | "card" | "cc" | "pan" => Scheme::CreditCard,
            "imei" => Scheme::Imei,
            "npi" => Scheme::Npi,
            "isbn10" => Scheme::Isbn10,
            "isbn13" => Scheme::Isbn13,
            "issn" => Scheme::Issn,
            "ean8" | "gtin8" => Scheme::Ean8,
            "ean13" | "gtin13" | "ean" => Scheme::Ean13,
            "upca" | "upc" | "gtin12" => Scheme::UpcA,
            "gtin14" | "itf14" | "scc14" => Scheme::Gtin14,
            "sscc" | "sscc18" => Scheme::Sscc,
            "iban" => Scheme::Iban,
            "routing" | "aba" | "routingnumber" | "abartn" => Scheme::Routing,
            "isin" => Scheme::Isin,
            "vin" => Scheme::Vin,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The code already carries its check digit — say whether it is correct.
    Validate,
    /// The code is the payload WITHOUT its check digit — compute it.
    Compute,
}

impl Mode {
    pub fn parse(s: &str) -> Option<Mode> {
        match s.trim().to_ascii_lowercase().as_str() {
            "validate" | "check" | "verify" => Some(Mode::Validate),
            "compute" | "calculate" | "generate" => Some(Mode::Compute),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// The code exactly as the user typed it (trimmed).
    pub input: String,
    /// Uppercased, separators removed.
    pub normalized: String,
    /// The scheme this verdict is about (None when the item errored).
    pub scheme: Option<Scheme>,
    pub valid: bool,
    /// The check character(s) the scheme requires.
    pub expected_check: Option<String>,
    /// The check character(s) actually present (validate mode).
    pub actual_check: Option<String>,
    /// The completed code (compute mode).
    pub full_code: Option<String>,
    /// Extra scheme-specific context: card brand, IBAN country, …
    pub detail: Option<String>,
    /// Other auto-detected schemes this code ALSO passes.
    pub also_valid_as: Vec<Scheme>,
    /// Arithmetic breakdown, when `show_steps` is on.
    pub steps: Option<String>,
    /// Per-item failure (bad characters, wrong length for the chosen scheme, …).
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub mode: Mode,
    /// The requested scheme, or None for auto-detect.
    pub requested_scheme: Option<Scheme>,
    pub items: Vec<Item>,
    pub total: usize,
    pub valid_count: usize,
    pub invalid_count: usize,
    pub error_count: usize,
}

impl Report {
    /// The plain-text report shown on the page and in the CLI `report` field.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        let noun = if self.total == 1 { "code" } else { "codes" };
        match self.mode {
            Mode::Validate => {
                out.push_str(&format!(
                    "Checked {} {noun} — {} valid, {} invalid",
                    self.total, self.valid_count, self.invalid_count
                ));
            }
            Mode::Compute => {
                out.push_str(&format!(
                    "Computed check digits for {} {noun} — {} succeeded",
                    self.total, self.valid_count
                ));
            }
        }
        if self.error_count > 0 {
            out.push_str(&format!(", {} could not be read", self.error_count));
        }
        out.push_str(".\n");

        for it in &self.items {
            out.push('\n');
            out.push_str(&it.input);
            out.push('\n');
            if let Some(err) = &it.error {
                out.push_str(&format!("  ERROR — {err}\n"));
                continue;
            }
            let label = it.scheme.map(|s| s.label()).unwrap_or("unknown");
            match self.mode {
                Mode::Validate => {
                    let verdict = if it.valid { "VALID" } else { "INVALID" };
                    out.push_str(&format!("  {verdict} — {label}"));
                    if let Some(d) = &it.detail {
                        out.push_str(&format!(" · {d}"));
                    }
                    out.push('\n');
                    if !it.valid {
                        if let (Some(exp), Some(got)) = (&it.expected_check, &it.actual_check) {
                            out.push_str(&format!(
                                "  expected check digit {exp}, got {got}\n"
                            ));
                        }
                    }
                    if !it.also_valid_as.is_empty() {
                        let others: Vec<&str> =
                            it.also_valid_as.iter().map(|s| s.label()).collect();
                        out.push_str(&format!("  also valid as: {}\n", others.join(", ")));
                    }
                }
                Mode::Compute => {
                    if let Some(exp) = &it.expected_check {
                        out.push_str(&format!("  {label} check digit: {exp}\n"));
                    }
                    if let Some(full) = &it.full_code {
                        out.push_str(&format!("  full code: {full}\n"));
                    }
                    if let Some(d) = &it.detail {
                        out.push_str(&format!("  {d}\n"));
                    }
                }
            }
            if let Some(s) = &it.steps {
                out.push_str(&format!("  steps: {s}\n"));
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Validate or compute check digits for one or more codes.
///
/// * `input` — one or more codes separated by newlines, commas or semicolons.
/// * `scheme` — `"auto"` or one of [`SCHEME_VALUES`].
/// * `mode` — `"validate"` or `"compute"`.
/// * `show_steps` — append the arithmetic breakdown to each item.
pub fn run(input: &str, scheme: &str, mode: &str, show_steps: bool) -> Result<Report, String> {
    let mode = Mode::parse(mode)
        .ok_or_else(|| format!("unknown mode '{mode}' (expected 'validate' or 'compute')"))?;

    let scheme_trimmed = scheme.trim();
    let requested = if scheme_trimmed.is_empty()
        || scheme_trimmed.eq_ignore_ascii_case("auto")
        || scheme_trimmed.eq_ignore_ascii_case("detect")
    {
        None
    } else {
        Some(Scheme::parse(scheme_trimmed).ok_or_else(|| {
            format!(
                "unknown scheme '{scheme_trimmed}' (expected auto or one of: {})",
                SCHEME_VALUES.join(", ")
            )
        })?)
    };

    if mode == Mode::Compute && requested.is_none() {
        return Err(
            "compute mode needs an explicit scheme — auto-detection reads the check digit that \
             compute mode is meant to produce. Pick a scheme (e.g. ean13 or isbn13)."
                .into(),
        );
    }

    let codes: Vec<&str> = input
        .split(|c| c == '\n' || c == '\r' || c == ',' || c == ';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if codes.is_empty() {
        return Err("no code to check — paste a number or code first".into());
    }
    if codes.len() > MAX_ITEMS {
        return Err(format!(
            "too many codes: {} (limit is {MAX_ITEMS} per run). Split the list into smaller batches.",
            codes.len()
        ));
    }

    let mut items = Vec::with_capacity(codes.len());
    for raw in codes {
        items.push(check_one(raw, requested, mode, show_steps));
    }

    let valid_count = items.iter().filter(|i| i.error.is_none() && i.valid).count();
    let error_count = items.iter().filter(|i| i.error.is_some()).count();
    let total = items.len();
    Ok(Report {
        mode,
        requested_scheme: requested,
        total,
        valid_count,
        invalid_count: total - valid_count - error_count,
        error_count,
        items,
    })
}

fn check_one(raw: &str, requested: Option<Scheme>, mode: Mode, show_steps: bool) -> Item {
    let mut item = Item {
        input: raw.to_string(),
        normalized: String::new(),
        scheme: requested,
        valid: false,
        expected_check: None,
        actual_check: None,
        full_code: None,
        detail: None,
        also_valid_as: Vec::new(),
        steps: None,
        error: None,
    };

    let code = match normalize(raw) {
        Ok(c) => c,
        Err(e) => {
            item.error = Some(e);
            return item;
        }
    };
    item.normalized = code.clone();

    if mode == Mode::Compute {
        // `requested` is guaranteed Some by run().
        let scheme = requested.expect("compute mode requires a scheme");
        match compute(scheme, &code) {
            Ok(c) => {
                item.valid = true;
                item.expected_check = Some(c.check);
                item.full_code = Some(c.full);
                item.detail = c.detail;
                if show_steps {
                    item.steps = Some(c.steps);
                }
            }
            Err(e) => item.error = Some(e),
        }
        return item;
    }

    match requested {
        Some(scheme) => match verify(scheme, &code) {
            Ok(v) => apply(&mut item, scheme, v, show_steps),
            Err(e) => item.error = Some(e),
        },
        None => {
            let cands = candidates(&code);
            if cands.is_empty() {
                item.error = Some(format!(
                    "'{code}' does not match the shape of any known scheme — pick a scheme \
                     explicitly (one of: {})",
                    SCHEME_VALUES.join(", ")
                ));
                return item;
            }
            // Verify every candidate; report the first PASSING one as the
            // primary match, else the most specific candidate.
            let mut verdicts = Vec::new();
            for s in cands {
                if let Ok(v) = verify(s, &code) {
                    verdicts.push((s, v));
                }
            }
            if verdicts.is_empty() {
                item.error = Some(format!("'{code}' could not be checked against any scheme"));
                return item;
            }
            let primary = verdicts
                .iter()
                .position(|(_, v)| v.valid)
                .unwrap_or(0);
            let (scheme, verdict) = verdicts.remove(primary);
            item.also_valid_as = verdicts
                .into_iter()
                .filter(|(_, v)| v.valid)
                .map(|(s, _)| s)
                .collect();
            apply(&mut item, scheme, verdict, show_steps);
        }
    }
    item
}

fn apply(item: &mut Item, scheme: Scheme, v: Verdict, show_steps: bool) {
    item.scheme = Some(scheme);
    item.valid = v.valid;
    item.expected_check = Some(v.expected);
    item.actual_check = Some(v.actual);
    item.detail = v.detail;
    if show_steps {
        item.steps = Some(v.steps);
    }
}

/// Uppercase and drop the allowed separators; reject anything else.
fn normalize(raw: &str) -> Result<String, String> {
    let mut out = String::new();
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
        } else if SEPARATORS.contains(&c) {
            continue;
        } else {
            return Err(format!(
                "unexpected character '{c}' (only letters, digits, spaces and - . _ / : are allowed)"
            ));
        }
    }
    if out.is_empty() {
        return Err("no letters or digits found".into());
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Auto-detection
// ---------------------------------------------------------------------------

/// Schemes whose shape (length + character set) fits this code, most specific
/// first. Every candidate is then actually verified.
fn candidates(code: &str) -> Vec<Scheme> {
    let n = code.len();
    let all_digits = code.bytes().all(|b| b.is_ascii_digit());
    let mut out = Vec::new();

    if !all_digits {
        // Letter-bearing codes.
        let b = code.as_bytes();
        if (15..=34).contains(&n)
            && b[0].is_ascii_uppercase()
            && b[1].is_ascii_uppercase()
            && b[2].is_ascii_digit()
            && b[3].is_ascii_digit()
        {
            out.push(Scheme::Iban);
        }
        if n == 17 && code.bytes().all(|c| !matches!(c, b'I' | b'O' | b'Q')) {
            out.push(Scheme::Vin);
        }
        if n == 12 && b[0].is_ascii_uppercase() && b[1].is_ascii_uppercase() {
            out.push(Scheme::Isin);
        }
        if n == 8 && b[..7].iter().all(|c| c.is_ascii_digit()) && b[7] == b'X' {
            out.push(Scheme::Issn);
        }
        if n == 10 && b[..9].iter().all(|c| c.is_ascii_digit()) && b[9] == b'X' {
            out.push(Scheme::Isbn10);
        }
        return out;
    }

    // Digits only — order matters: most specific scheme first.
    match n {
        8 => {
            out.push(Scheme::Ean8);
            out.push(Scheme::Issn);
        }
        9 => out.push(Scheme::Routing),
        10 => {
            out.push(Scheme::Isbn10);
            if code.starts_with('1') || code.starts_with('2') {
                out.push(Scheme::Npi);
            }
        }
        12 => out.push(Scheme::UpcA),
        13 => {
            if code.starts_with("978") || code.starts_with("979") {
                out.push(Scheme::Isbn13);
            }
            out.push(Scheme::Ean13);
        }
        14 => out.push(Scheme::Gtin14),
        15 => out.push(Scheme::Imei),
        18 => out.push(Scheme::Sscc),
        _ => {}
    }
    if (12..=19).contains(&n) {
        out.push(Scheme::CreditCard);
    }
    if n >= 2 {
        out.push(Scheme::Luhn);
    }
    out
}

// ---------------------------------------------------------------------------
// Verification
// ---------------------------------------------------------------------------

struct Verdict {
    valid: bool,
    expected: String,
    actual: String,
    detail: Option<String>,
    steps: String,
}

struct Computed {
    check: String,
    full: String,
    detail: Option<String>,
    steps: String,
}

fn digits(code: &str) -> Result<Vec<u8>, String> {
    code.bytes()
        .map(|b| {
            if b.is_ascii_digit() {
                Ok(b - b'0')
            } else {
                Err(format!(
                    "'{}' is not a digit — this scheme accepts digits only",
                    b as char
                ))
            }
        })
        .collect()
}

fn need_len(code: &str, want: usize, what: &str) -> Result<(), String> {
    if code.len() != want {
        return Err(format!(
            "{what} needs exactly {want} characters, got {}",
            code.len()
        ));
    }
    Ok(())
}

/// Luhn total with the RIGHTMOST digit weighted ×1 (i.e. the string already
/// carries its check digit). Valid when the total is a multiple of 10.
fn luhn_total(ds: &[u8]) -> u32 {
    ds.iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| {
            let mut v = d as u32;
            if i % 2 == 1 {
                v *= 2;
                if v > 9 {
                    v -= 9;
                }
            }
            v
        })
        .sum()
}

/// The Luhn check digit that completes `payload` (payload carries NO check digit).
fn luhn_check(payload: &[u8]) -> (u8, u32) {
    let mut with_zero = payload.to_vec();
    with_zero.push(0);
    let total = luhn_total(&with_zero);
    (((10 - total % 10) % 10) as u8, total)
}

/// GS1 weighted sum for a payload WITHOUT its check digit: ×3 / ×1 alternating
/// from the right.
fn gs1_sum(payload: &[u8]) -> u32 {
    payload
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| d as u32 * if i % 2 == 0 { 3 } else { 1 })
        .sum()
}

fn gs1_check(payload: &[u8]) -> (u8, u32) {
    let sum = gs1_sum(payload);
    (((10 - sum % 10) % 10) as u8, sum)
}

fn verify_gs1(code: &str, len: usize, what: &str) -> Result<Verdict, String> {
    need_len(code, len, what)?;
    let ds = digits(code)?;
    let (expected, sum) = gs1_check(&ds[..len - 1]);
    let actual = ds[len - 1];
    Ok(Verdict {
        valid: expected == actual,
        expected: expected.to_string(),
        actual: actual.to_string(),
        detail: None,
        steps: format!(
            "GS1 weighted sum (×3/×1 from the right, check digit excluded) = {sum}; \
             (10 − {sum} mod 10) mod 10 = {expected}"
        ),
    })
}

fn compute_gs1(payload: &str, len: usize, what: &str) -> Result<Computed, String> {
    need_len(payload, len - 1, &format!("{what} payload (without its check digit)"))?;
    let ds = digits(payload)?;
    let (check, sum) = gs1_check(&ds);
    Ok(Computed {
        check: check.to_string(),
        full: format!("{payload}{check}"),
        detail: None,
        steps: format!(
            "GS1 weighted sum (×3/×1 from the right) = {sum}; (10 − {sum} mod 10) mod 10 = {check}"
        ),
    })
}

fn verify_luhn_family(code: &str, scheme: Scheme) -> Result<Verdict, String> {
    match scheme {
        Scheme::Imei => need_len(code, 15, "an IMEI")?,
        Scheme::Npi => need_len(code, 10, "an NPI")?,
        Scheme::CreditCard => {
            if !(12..=19).contains(&code.len()) {
                return Err(format!(
                    "a payment card number is 12–19 digits, got {}",
                    code.len()
                ));
            }
        }
        _ => {
            if code.len() < 2 {
                return Err("a Luhn check needs at least 2 digits".into());
            }
        }
    }
    let ds = digits(code)?;
    // NPI runs Luhn over the constant 80840 prefix + the 10-digit NPI.
    let full: Vec<u8> = if scheme == Scheme::Npi {
        let mut v = vec![8, 0, 8, 4, 0];
        v.extend_from_slice(&ds);
        v
    } else {
        ds.clone()
    };
    let payload = &full[..full.len() - 1];
    let (expected, _) = luhn_check(payload);
    let actual = ds[ds.len() - 1];
    let total = luhn_total(&full);
    let detail = if scheme == Scheme::CreditCard {
        card_brand(code).map(|b| b.to_string())
    } else {
        None
    };
    let prefix = if scheme == Scheme::Npi {
        "80840 prefix + NPI: "
    } else {
        ""
    };
    Ok(Verdict {
        valid: expected == actual,
        expected: expected.to_string(),
        actual: actual.to_string(),
        detail,
        steps: format!(
            "{prefix}every 2nd digit from the right doubled (9 subtracted when >9); \
             sum = {total}; {total} mod 10 = {} (must be 0)",
            total % 10
        ),
    })
}

fn compute_luhn_family(payload: &str, scheme: Scheme) -> Result<Computed, String> {
    match scheme {
        Scheme::Imei => need_len(payload, 14, "an IMEI payload (without its check digit)")?,
        Scheme::Npi => need_len(payload, 9, "an NPI payload (without its check digit)")?,
        _ => {
            if payload.is_empty() {
                return Err("need at least 1 digit to compute a Luhn check digit".into());
            }
        }
    }
    let ds = digits(payload)?;
    let full: Vec<u8> = if scheme == Scheme::Npi {
        let mut v = vec![8, 0, 8, 4, 0];
        v.extend_from_slice(&ds);
        v
    } else {
        ds.clone()
    };
    let (check, total) = luhn_check(&full);
    let completed = format!("{payload}{check}");
    let detail = if scheme == Scheme::CreditCard {
        card_brand(&completed).map(|b| b.to_string())
    } else {
        None
    };
    Ok(Computed {
        check: check.to_string(),
        full: completed,
        detail,
        steps: format!(
            "Luhn sum with a trailing 0 = {total}; (10 − {total} mod 10) mod 10 = {check}"
        ),
    })
}

fn verify_isbn10(code: &str) -> Result<Verdict, String> {
    need_len(code, 10, "an ISBN-10")?;
    let b = code.as_bytes();
    if !b[..9].iter().all(|c| c.is_ascii_digit()) {
        return Err("an ISBN-10's first 9 characters must be digits".into());
    }
    let actual = match b[9] {
        b'X' => 10u32,
        d if d.is_ascii_digit() => (d - b'0') as u32,
        other => {
            return Err(format!(
                "'{}' is not a valid ISBN-10 check character (0-9 or X)",
                other as char
            ))
        }
    };
    let sum: u32 = b[..9]
        .iter()
        .enumerate()
        .map(|(i, &d)| (10 - i as u32) * (d - b'0') as u32)
        .sum();
    let expected = (11 - sum % 11) % 11;
    Ok(Verdict {
        valid: expected == actual,
        expected: mod11_char(expected),
        actual: mod11_char(actual),
        detail: None,
        steps: format!(
            "Σ position-weighted digits (×10…×2) = {sum}; (11 − {sum} mod 11) mod 11 = {}",
            mod11_char(expected)
        ),
    })
}

fn compute_isbn10(payload: &str) -> Result<Computed, String> {
    need_len(payload, 9, "an ISBN-10 payload (without its check digit)")?;
    let ds = digits(payload)?;
    let sum: u32 = ds
        .iter()
        .enumerate()
        .map(|(i, &d)| (10 - i as u32) * d as u32)
        .sum();
    let expected = (11 - sum % 11) % 11;
    let check = mod11_char(expected);
    Ok(Computed {
        check: check.clone(),
        full: format!("{payload}{check}"),
        detail: None,
        steps: format!("Σ position-weighted digits (×10…×2) = {sum}; (11 − {sum} mod 11) mod 11 = {check}"),
    })
}

fn verify_issn(code: &str) -> Result<Verdict, String> {
    need_len(code, 8, "an ISSN")?;
    let b = code.as_bytes();
    if !b[..7].iter().all(|c| c.is_ascii_digit()) {
        return Err("an ISSN's first 7 characters must be digits".into());
    }
    let actual = match b[7] {
        b'X' => 10u32,
        d if d.is_ascii_digit() => (d - b'0') as u32,
        other => {
            return Err(format!(
                "'{}' is not a valid ISSN check character (0-9 or X)",
                other as char
            ))
        }
    };
    let sum: u32 = b[..7]
        .iter()
        .enumerate()
        .map(|(i, &d)| (8 - i as u32) * (d - b'0') as u32)
        .sum();
    let expected = (11 - sum % 11) % 11;
    Ok(Verdict {
        valid: expected == actual,
        expected: mod11_char(expected),
        actual: mod11_char(actual),
        detail: None,
        steps: format!(
            "Σ weighted digits (×8…×2) = {sum}; (11 − {sum} mod 11) mod 11 = {}",
            mod11_char(expected)
        ),
    })
}

fn compute_issn(payload: &str) -> Result<Computed, String> {
    need_len(payload, 7, "an ISSN payload (without its check digit)")?;
    let ds = digits(payload)?;
    let sum: u32 = ds
        .iter()
        .enumerate()
        .map(|(i, &d)| (8 - i as u32) * d as u32)
        .sum();
    let check = mod11_char((11 - sum % 11) % 11);
    Ok(Computed {
        check: check.clone(),
        full: format!("{payload}{check}"),
        detail: None,
        steps: format!("Σ weighted digits (×8…×2) = {sum}; (11 − {sum} mod 11) mod 11 = {check}"),
    })
}

fn mod11_char(v: u32) -> String {
    if v == 10 {
        "X".to_string()
    } else {
        v.to_string()
    }
}

fn verify_routing(code: &str) -> Result<Verdict, String> {
    need_len(code, 9, "an ABA routing number")?;
    let d = digits(code)?;
    let sum = 3 * (d[0] as u32 + d[3] as u32 + d[6] as u32)
        + 7 * (d[1] as u32 + d[4] as u32 + d[7] as u32)
        + (d[2] as u32 + d[5] as u32 + d[8] as u32);
    let head: u32 = 3 * (d[0] as u32 + d[3] as u32 + d[6] as u32)
        + 7 * (d[1] as u32 + d[4] as u32 + d[7] as u32)
        + (d[2] as u32 + d[5] as u32);
    let expected = (10 - head % 10) % 10;
    Ok(Verdict {
        valid: sum % 10 == 0,
        expected: expected.to_string(),
        actual: d[8].to_string(),
        detail: Some(format!("Federal Reserve routing symbol {}", &code[..4])),
        steps: format!(
            "3(d1+d4+d7) + 7(d2+d5+d8) + (d3+d6+d9) = {sum}; {sum} mod 10 = {} (must be 0)",
            sum % 10
        ),
    })
}

fn compute_routing(payload: &str) -> Result<Computed, String> {
    need_len(payload, 8, "an ABA routing payload (without its check digit)")?;
    let d = digits(payload)?;
    let head: u32 = 3 * (d[0] as u32 + d[3] as u32 + d[6] as u32)
        + 7 * (d[1] as u32 + d[4] as u32 + d[7] as u32)
        + (d[2] as u32 + d[5] as u32);
    let check = (10 - head % 10) % 10;
    Ok(Computed {
        check: check.to_string(),
        full: format!("{payload}{check}"),
        detail: None,
        steps: format!("3·7·1 weighted sum of the first 8 digits = {head}; (10 − {head} mod 10) mod 10 = {check}"),
    })
}

fn alnum_value(c: u8) -> Option<u32> {
    if c.is_ascii_digit() {
        Some((c - b'0') as u32)
    } else if c.is_ascii_uppercase() {
        Some((c - b'A') as u32 + 10)
    } else {
        None
    }
}

/// ISO 7064 mod-97-10 over an alphanumeric string (letters expand to 2 digits).
fn mod97(s: &str) -> Result<u32, String> {
    let mut rem: u32 = 0;
    for c in s.bytes() {
        let v = alnum_value(c)
            .ok_or_else(|| format!("'{}' is not allowed in an IBAN", c as char))?;
        rem = if v >= 10 {
            (rem * 100 + v) % 97
        } else {
            (rem * 10 + v) % 97
        };
    }
    Ok(rem)
}

fn verify_iban(code: &str) -> Result<Verdict, String> {
    if !(15..=34).contains(&code.len()) {
        return Err(format!(
            "an IBAN is 15–34 characters, got {}",
            code.len()
        ));
    }
    let b = code.as_bytes();
    if !(b[0].is_ascii_uppercase() && b[1].is_ascii_uppercase()) {
        return Err("an IBAN starts with a 2-letter country code".into());
    }
    if !(b[2].is_ascii_digit() && b[3].is_ascii_digit()) {
        return Err("an IBAN's 3rd and 4th characters are its two check digits".into());
    }
    let cc = &code[..2];
    let rearranged = format!("{}{}", &code[4..], &code[..4]);
    let rem = mod97(&rearranged)?;
    let expected = 98 - mod97(&format!("{}{}00", &code[4..], cc))?;
    let mut detail = match iban_country(cc) {
        Some((len, name)) => {
            if len != code.len() {
                return Ok(Verdict {
                    valid: false,
                    expected: format!("{expected:02}"),
                    actual: code[2..4].to_string(),
                    detail: Some(format!(
                        "{name} IBANs are {len} characters, this one is {}",
                        code.len()
                    )),
                    steps: "length check failed before the mod-97 test".to_string(),
                });
            }
            format!("{name}, {len} characters")
        }
        None => format!("country '{cc}' is not in the built-in registry — length not checked"),
    };
    if rem == 1 {
        detail.push_str(&format!(", BBAN {}", &code[4..]));
    }
    Ok(Verdict {
        valid: rem == 1,
        expected: format!("{expected:02}"),
        actual: code[2..4].to_string(),
        detail: Some(detail),
        steps: format!("first 4 characters moved to the end → {rearranged}; mod 97 = {rem} (must be 1)"),
    })
}

fn compute_iban(input: &str) -> Result<Computed, String> {
    let b = input.as_bytes();
    if input.len() < 6 || !(b[0].is_ascii_uppercase() && b[1].is_ascii_uppercase()) {
        return Err("an IBAN starts with a 2-letter country code followed by the account part".into());
    }
    let cc = &input[..2];
    // Accept either "CC" + BBAN (no check digits) or a full IBAN whose check
    // digits are a placeholder such as 00.
    let bban = if b[2].is_ascii_digit() && b[3].is_ascii_digit() {
        match iban_country(cc) {
            Some((len, _)) if input.len() == len - 2 => &input[2..],
            _ => &input[4..],
        }
    } else {
        &input[2..]
    };
    let expected = 98 - mod97(&format!("{bban}{cc}00"))?;
    let full = format!("{cc}{expected:02}{bban}");
    let detail = iban_country(cc).map(|(len, name)| {
        if len == full.len() {
            format!("{name}, {len} characters")
        } else {
            format!("{name} IBANs are {len} characters, this one is {}", full.len())
        }
    });
    Ok(Computed {
        check: format!("{expected:02}"),
        full,
        detail,
        steps: format!("98 − (mod 97 of {bban}{cc}00) = {expected:02}"),
    })
}

fn verify_isin(code: &str) -> Result<Verdict, String> {
    need_len(code, 12, "an ISIN")?;
    let b = code.as_bytes();
    if !(b[0].is_ascii_uppercase() && b[1].is_ascii_uppercase()) {
        return Err("an ISIN starts with a 2-letter country code".into());
    }
    if !b[11].is_ascii_digit() {
        return Err("an ISIN's check character is a digit".into());
    }
    let expanded = isin_expand(&code[..11])?;
    let mut ds = expanded.clone();
    ds.push(b[11] - b'0');
    let (expected, _) = luhn_check(&expanded);
    let total = luhn_total(&ds);
    Ok(Verdict {
        valid: total % 10 == 0,
        expected: expected.to_string(),
        actual: (b[11] - b'0').to_string(),
        detail: Some(format!("country prefix {}", &code[..2])),
        steps: format!(
            "letters expanded to 2-digit values, then Luhn; sum = {total}; {total} mod 10 = {} (must be 0)",
            total % 10
        ),
    })
}

fn compute_isin(payload: &str) -> Result<Computed, String> {
    need_len(payload, 11, "an ISIN payload (without its check digit)")?;
    let expanded = isin_expand(payload)?;
    let (check, total) = luhn_check(&expanded);
    Ok(Computed {
        check: check.to_string(),
        full: format!("{payload}{check}"),
        detail: None,
        steps: format!("letters expanded, Luhn sum with a trailing 0 = {total}; check digit = {check}"),
    })
}

fn isin_expand(s: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for c in s.bytes() {
        match alnum_value(c) {
            Some(v) if v < 10 => out.push(v as u8),
            Some(v) => {
                out.push((v / 10) as u8);
                out.push((v % 10) as u8);
            }
            None => {
                return Err(format!(
                    "'{}' is not allowed in an ISIN (A-Z and 0-9 only)",
                    c as char
                ))
            }
        }
    }
    Ok(out)
}

const VIN_WEIGHTS: [u32; 17] = [8, 7, 6, 5, 4, 3, 2, 10, 0, 9, 8, 7, 6, 5, 4, 3, 2];

fn vin_value(c: u8) -> Option<u32> {
    Some(match c {
        b'0'..=b'9' => (c - b'0') as u32,
        b'A' | b'J' => 1,
        b'B' | b'K' | b'S' => 2,
        b'C' | b'L' | b'T' => 3,
        b'D' | b'M' | b'U' => 4,
        b'E' | b'N' | b'V' => 5,
        b'F' | b'W' => 6,
        b'G' | b'P' | b'X' => 7,
        b'H' | b'Y' => 8,
        b'R' | b'Z' => 9,
        _ => return None,
    })
}

/// VIN's check character sits at position 9, not at the end — so both validate
/// and compute take the full 17-character VIN (use any placeholder at position 9
/// when computing).
fn verify_vin(code: &str) -> Result<Verdict, String> {
    need_len(code, 17, "a VIN")?;
    let b = code.as_bytes();
    let mut sum = 0u32;
    for (i, &c) in b.iter().enumerate() {
        if i == 8 {
            continue; // the check character itself
        }
        let v = vin_value(c).ok_or_else(|| {
            format!("'{}' is not allowed in a VIN (I, O and Q are excluded)", c as char)
        })?;
        sum += v * VIN_WEIGHTS[i];
    }
    let expected = mod11_char(sum % 11);
    let actual = (b[8] as char).to_string();
    Ok(Verdict {
        valid: expected == actual,
        expected,
        actual,
        detail: Some(format!("world manufacturer identifier {}", &code[..3])),
        steps: format!(
            "Σ transliterated value × position weight = {sum}; {sum} mod 11 = {} → check character at position 9",
            sum % 11
        ),
    })
}

fn compute_vin(code: &str) -> Result<Computed, String> {
    let v = verify_vin(code)?;
    let mut full = code.to_string();
    full.replace_range(8..9, &v.expected);
    Ok(Computed {
        check: v.expected,
        full,
        detail: Some("the VIN check character sits at position 9".into()),
        steps: v.steps,
    })
}

fn verify(scheme: Scheme, code: &str) -> Result<Verdict, String> {
    match scheme {
        Scheme::Luhn | Scheme::CreditCard | Scheme::Imei | Scheme::Npi => {
            verify_luhn_family(code, scheme)
        }
        Scheme::Isbn10 => verify_isbn10(code),
        Scheme::Issn => verify_issn(code),
        Scheme::Isbn13 => verify_gs1(code, 13, "an ISBN-13"),
        Scheme::Ean13 => verify_gs1(code, 13, "an EAN-13"),
        Scheme::Ean8 => verify_gs1(code, 8, "an EAN-8"),
        Scheme::UpcA => verify_gs1(code, 12, "a UPC-A"),
        Scheme::Gtin14 => verify_gs1(code, 14, "a GTIN-14"),
        Scheme::Sscc => verify_gs1(code, 18, "an SSCC-18"),
        Scheme::Iban => verify_iban(code),
        Scheme::Routing => verify_routing(code),
        Scheme::Isin => verify_isin(code),
        Scheme::Vin => verify_vin(code),
    }
}

fn compute(scheme: Scheme, payload: &str) -> Result<Computed, String> {
    match scheme {
        Scheme::Luhn | Scheme::CreditCard | Scheme::Imei | Scheme::Npi => {
            compute_luhn_family(payload, scheme)
        }
        Scheme::Isbn10 => compute_isbn10(payload),
        Scheme::Issn => compute_issn(payload),
        Scheme::Isbn13 => compute_gs1(payload, 13, "an ISBN-13"),
        Scheme::Ean13 => compute_gs1(payload, 13, "an EAN-13"),
        Scheme::Ean8 => compute_gs1(payload, 8, "an EAN-8"),
        Scheme::UpcA => compute_gs1(payload, 12, "a UPC-A"),
        Scheme::Gtin14 => compute_gs1(payload, 14, "a GTIN-14"),
        Scheme::Sscc => compute_gs1(payload, 18, "an SSCC-18"),
        Scheme::Iban => compute_iban(payload),
        Scheme::Routing => compute_routing(payload),
        Scheme::Isin => compute_isin(payload),
        Scheme::Vin => compute_vin(payload),
    }
}

// ---------------------------------------------------------------------------
// Reference data
// ---------------------------------------------------------------------------

/// Best-effort payment-card brand from prefix + length. A brand is only a hint —
/// it never means the account exists.
fn card_brand(code: &str) -> Option<&'static str> {
    let n = code.len();
    let p = |k: usize| -> u32 { code[..k.min(n)].parse().unwrap_or(0) };
    let p1 = p(1);
    let p2 = p(2);
    let p3 = p(3);
    let p4 = p(4);
    if p1 == 4 && matches!(n, 13 | 16 | 19) {
        return Some("Visa");
    }
    if ((51..=55).contains(&p2) || (2221..=2720).contains(&p4)) && n == 16 {
        return Some("Mastercard");
    }
    if (p2 == 34 || p2 == 37) && n == 15 {
        return Some("American Express");
    }
    if (p4 == 6011 || (644..=649).contains(&p3) || p2 == 65) && (16..=19).contains(&n) {
        return Some("Discover");
    }
    if (3528..=3589).contains(&p4) && (16..=19).contains(&n) {
        return Some("JCB");
    }
    if ((300..=305).contains(&p3) || p4 == 3095 || p2 == 36 || p2 == 38 || p2 == 39)
        && (14..=19).contains(&n)
    {
        return Some("Diners Club");
    }
    if p2 == 62 && (16..=19).contains(&n) {
        return Some("UnionPay");
    }
    if matches!(p4, 5018 | 5020 | 5038 | 5893 | 6304 | 6759 | 6761 | 6762 | 6763)
        && (12..=19).contains(&n)
    {
        return Some("Maestro");
    }
    None
}

/// IBAN registry: country code → (total length, country name).
fn iban_country(cc: &str) -> Option<(usize, &'static str)> {
    Some(match cc {
        "AD" => (24, "Andorra"),
        "AE" => (23, "United Arab Emirates"),
        "AL" => (28, "Albania"),
        "AT" => (20, "Austria"),
        "AZ" => (28, "Azerbaijan"),
        "BA" => (20, "Bosnia and Herzegovina"),
        "BE" => (16, "Belgium"),
        "BG" => (22, "Bulgaria"),
        "BH" => (22, "Bahrain"),
        "BI" => (27, "Burundi"),
        "BR" => (29, "Brazil"),
        "BY" => (28, "Belarus"),
        "CH" => (21, "Switzerland"),
        "CR" => (22, "Costa Rica"),
        "CY" => (28, "Cyprus"),
        "CZ" => (24, "Czechia"),
        "DE" => (22, "Germany"),
        "DJ" => (27, "Djibouti"),
        "DK" => (18, "Denmark"),
        "DO" => (28, "Dominican Republic"),
        "EE" => (20, "Estonia"),
        "EG" => (29, "Egypt"),
        "ES" => (24, "Spain"),
        "FI" => (18, "Finland"),
        "FK" => (18, "Falkland Islands"),
        "FO" => (18, "Faroe Islands"),
        "FR" => (27, "France"),
        "GB" => (22, "United Kingdom"),
        "GE" => (22, "Georgia"),
        "GI" => (23, "Gibraltar"),
        "GL" => (18, "Greenland"),
        "GR" => (27, "Greece"),
        "GT" => (28, "Guatemala"),
        "HN" => (28, "Honduras"),
        "HR" => (21, "Croatia"),
        "HU" => (28, "Hungary"),
        "IE" => (22, "Ireland"),
        "IL" => (23, "Israel"),
        "IQ" => (23, "Iraq"),
        "IS" => (26, "Iceland"),
        "IT" => (27, "Italy"),
        "JO" => (30, "Jordan"),
        "KW" => (30, "Kuwait"),
        "KZ" => (20, "Kazakhstan"),
        "LB" => (28, "Lebanon"),
        "LC" => (32, "Saint Lucia"),
        "LI" => (21, "Liechtenstein"),
        "LT" => (20, "Lithuania"),
        "LU" => (20, "Luxembourg"),
        "LV" => (21, "Latvia"),
        "LY" => (25, "Libya"),
        "MC" => (27, "Monaco"),
        "MD" => (24, "Moldova"),
        "ME" => (22, "Montenegro"),
        "MK" => (19, "North Macedonia"),
        "MN" => (20, "Mongolia"),
        "MR" => (27, "Mauritania"),
        "MT" => (31, "Malta"),
        "MU" => (30, "Mauritius"),
        "NI" => (28, "Nicaragua"),
        "NL" => (18, "Netherlands"),
        "NO" => (15, "Norway"),
        "OM" => (23, "Oman"),
        "PK" => (24, "Pakistan"),
        "PL" => (28, "Poland"),
        "PS" => (29, "Palestine"),
        "PT" => (25, "Portugal"),
        "QA" => (29, "Qatar"),
        "RO" => (24, "Romania"),
        "RS" => (22, "Serbia"),
        "RU" => (33, "Russia"),
        "SA" => (24, "Saudi Arabia"),
        "SC" => (31, "Seychelles"),
        "SD" => (18, "Sudan"),
        "SE" => (24, "Sweden"),
        "SI" => (19, "Slovenia"),
        "SK" => (24, "Slovakia"),
        "SM" => (27, "San Marino"),
        "SO" => (23, "Somalia"),
        "ST" => (25, "Sao Tome and Principe"),
        "SV" => (28, "El Salvador"),
        "TL" => (23, "Timor-Leste"),
        "TN" => (24, "Tunisia"),
        "TR" => (26, "Turkey"),
        "UA" => (29, "Ukraine"),
        "VA" => (22, "Vatican City"),
        "VG" => (24, "British Virgin Islands"),
        "XK" => (20, "Kosovo"),
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn one(input: &str, scheme: &str) -> Item {
        run(input, scheme, "validate", false).unwrap().items.remove(0)
    }

    #[test]
    fn auto_detects_a_valid_visa_test_number() {
        let it = one("4539 1488 0343 6467", "auto");
        assert!(it.valid, "{it:?}");
        assert_eq!(it.scheme, Some(Scheme::CreditCard));
        assert_eq!(it.detail.as_deref(), Some("Visa"));
        assert_eq!(it.normalized, "4539148803436467");
    }

    #[test]
    fn rejects_a_bad_character() {
        let it = one("4539*1488", "auto");
        assert!(it.error.as_ref().unwrap().contains("unexpected character '*'"));
    }

    #[test]
    fn isbn13_and_ean13_both_match_a_book_barcode() {
        let it = one("978-0-306-40615-7", "auto");
        assert!(it.valid);
        assert_eq!(it.scheme, Some(Scheme::Isbn13));
        assert!(it.also_valid_as.contains(&Scheme::Ean13));
    }

    #[test]
    fn isbn13_typo_reports_the_expected_digit() {
        let it = one("9780306406158", "isbn13");
        assert!(!it.valid);
        assert_eq!(it.expected_check.as_deref(), Some("7"));
        assert_eq!(it.actual_check.as_deref(), Some("8"));
    }

    #[test]
    fn isbn10_with_x_check_digit() {
        let it = one("0-8044-2957-X", "auto");
        assert!(it.valid, "{it:?}");
        assert_eq!(it.scheme, Some(Scheme::Isbn10));
    }

    #[test]
    fn issn_with_x_check_digit() {
        let it = one("0378-5955", "issn");
        assert!(it.valid, "{it:?}");
        let it = one("2049-3630", "issn");
        assert!(it.valid, "{it:?}");
    }

    #[test]
    fn ean8_upca_gtin14_and_sscc() {
        assert!(one("96385074", "ean8").valid);
        assert!(one("036000291452", "upc-a").valid);
        assert!(one("10614141000415", "gtin14").valid);
        assert!(one("340123450000000017", "sscc").valid);
    }

    #[test]
    fn iban_validates_and_names_the_country() {
        let it = one("GB82 WEST 1234 5698 7654 32", "auto");
        assert!(it.valid, "{it:?}");
        assert_eq!(it.scheme, Some(Scheme::Iban));
        assert!(it.detail.as_ref().unwrap().contains("United Kingdom"));
    }

    #[test]
    fn iban_with_a_bad_check_digit_reports_the_right_one() {
        let it = one("GB83WEST12345698765432", "iban");
        assert!(!it.valid);
        assert_eq!(it.expected_check.as_deref(), Some("82"));
        assert_eq!(it.actual_check.as_deref(), Some("83"));
    }

    #[test]
    fn iban_wrong_length_for_country_is_invalid() {
        let it = one("GB82WEST123456987654", "iban");
        assert!(!it.valid);
        assert!(it.detail.as_ref().unwrap().contains("22 characters"));
    }

    #[test]
    fn routing_number_check() {
        let it = one("021000021", "routing");
        assert!(it.valid, "{it:?}");
        assert!(!one("021000022", "routing").valid);
    }

    #[test]
    fn isin_and_vin() {
        let it = one("US0378331005", "isin");
        assert!(it.valid, "{it:?}");
        assert!(!one("US0378331006", "isin").valid);
        let it = one("1HGCM82633A004352", "vin");
        assert!(it.valid, "{it:?}");
        assert_eq!(it.expected_check.as_deref(), Some("3"));
    }

    #[test]
    fn imei_and_npi() {
        assert!(one("490154203237518", "imei").valid);
        assert!(one("1234567893", "npi").valid);
        assert!(!one("1234567890", "npi").valid);
    }

    #[test]
    fn compute_mode_appends_the_check_digit() {
        let r = run("978030640615", "isbn13", "compute", false).unwrap();
        assert_eq!(r.items[0].expected_check.as_deref(), Some("7"));
        assert_eq!(r.items[0].full_code.as_deref(), Some("9780306406157"));
    }

    #[test]
    fn compute_mode_needs_an_explicit_scheme() {
        let e = run("978030640615", "auto", "compute", false).unwrap_err();
        assert!(e.contains("compute mode needs an explicit scheme"));
    }

    #[test]
    fn compute_iban_from_country_plus_bban() {
        let r = run("GBWEST12345698765432", "iban", "compute", false).unwrap();
        assert_eq!(r.items[0].full_code.as_deref(), Some("GB82WEST12345698765432"));
        // A full IBAN with placeholder check digits works too.
        let r = run("GB00WEST12345698765432", "iban", "compute", false).unwrap();
        assert_eq!(r.items[0].full_code.as_deref(), Some("GB82WEST12345698765432"));
    }

    #[test]
    fn compute_vin_replaces_position_nine() {
        // Position 9 is the check character, so compute takes the full VIN and
        // rewrites that one position (here it was already correct).
        let r = run("1HGCM82603A004352", "vin", "compute", false).unwrap();
        assert_eq!(r.items[0].expected_check.as_deref(), Some("3"));
        assert_eq!(r.items[0].full_code.as_deref(), Some("1HGCM82633A004352"));
        let r = run("1HGCM8260", "vin", "compute", false).unwrap();
        assert!(r.items[0].error.is_some());
    }

    #[test]
    fn batch_counts_and_cap() {
        let r = run("4539148803436467\n9780306406157\n021000021", "auto", "validate", false)
            .unwrap();
        assert_eq!(r.total, 3);
        assert_eq!(r.valid_count, 3);
        let many = vec!["4539148803436467"; MAX_ITEMS].join("\n");
        assert_eq!(run(&many, "auto", "validate", false).unwrap().total, MAX_ITEMS);
        let too_many = vec!["4539148803436467"; MAX_ITEMS + 1].join("\n");
        let e = run(&too_many, "auto", "validate", false).unwrap_err();
        assert!(e.contains("too many codes"));
    }

    #[test]
    fn steps_are_shown_only_when_asked() {
        let r = run("9780306406157", "isbn13", "validate", true).unwrap();
        assert!(r.items[0].steps.as_ref().unwrap().contains("GS1 weighted sum"));
        let r = run("9780306406157", "isbn13", "validate", false).unwrap();
        assert!(r.items[0].steps.is_none());
    }

    #[test]
    fn unknown_scheme_and_mode_are_rejected() {
        assert!(run("123", "mod97", "validate", false)
            .unwrap_err()
            .contains("unknown scheme"));
        assert!(run("123", "auto", "sideways", false)
            .unwrap_err()
            .contains("unknown mode"));
        assert!(run("   ", "auto", "validate", false)
            .unwrap_err()
            .contains("no code to check"));
    }

    #[test]
    fn wrong_length_for_an_explicit_scheme_is_a_per_item_error() {
        let it = one("97803064061", "isbn13");
        assert!(it
            .error
            .as_ref()
            .unwrap()
            .contains("needs exactly 13 characters"));
    }

    #[test]
    fn report_text_has_a_summary_and_a_verdict_line() {
        let t = run("9780306406157", "auto", "validate", false)
            .unwrap()
            .to_text();
        assert!(t.starts_with("Checked 1 code — 1 valid, 0 invalid."));
        assert!(t.contains("VALID — ISBN-13 (GS1 mod-10)"));
    }
}
