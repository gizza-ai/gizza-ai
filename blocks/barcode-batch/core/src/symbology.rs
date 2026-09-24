//! Symbology selection, input normalisation and check-digit maths.
//!
//! Everything here turns a pasted string into (a) the module vector the
//! renderers draw and (b) the human-readable interpretation (HRI) text printed
//! under the symbol. The HRI is always the *encoded* value — including any check
//! digit this module computed — never an arbitrary caption, which is what the
//! GS1 / ISO symbology specs require.

use barcoders::sym::codabar::Codabar;
use barcoders::sym::code128::Code128;
use barcoders::sym::code39::Code39;
use barcoders::sym::code93::Code93;
use barcoders::sym::ean13::{EAN13, UPCA};
use barcoders::sym::ean8::EAN8;
use barcoders::sym::tf::TF;

/// The 1D symbologies this tool can emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Symbology {
    /// Pick per row from the value's shape (see [`Symbology::resolve`]).
    Auto,
    Code128,
    Code39,
    Code93,
    Ean13,
    Ean8,
    UpcA,
    /// Interleaved 2 of 5 — the ITF-14 shipping-carton symbology.
    Itf,
    Codabar,
}

impl Symbology {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().replace(['-', '_', ' '], "").as_str() {
            "" | "auto" => Ok(Self::Auto),
            "code128" => Ok(Self::Code128),
            "code39" => Ok(Self::Code39),
            "code93" => Ok(Self::Code93),
            "ean13" => Ok(Self::Ean13),
            "ean8" => Ok(Self::Ean8),
            "upca" => Ok(Self::UpcA),
            "itf" | "itf14" => Ok(Self::Itf),
            "codabar" => Ok(Self::Codabar),
            other => Err(format!(
                "unknown symbology `{other}` — use auto, code128, code39, code93, ean13, ean8, upca, itf or codabar"
            )),
        }
    }

    /// Short label used in index.csv and error messages.
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Code128 => "Code 128",
            Self::Code39 => "Code 39",
            Self::Code93 => "Code 93",
            Self::Ean13 => "EAN-13",
            Self::Ean8 => "EAN-8",
            Self::UpcA => "UPC-A",
            Self::Itf => "ITF",
            Self::Codabar => "Codabar",
        }
    }

    /// Resolve `auto` against one row's value.
    ///
    /// Digit-only values map to the retail symbology of that length; 12 digits is
    /// read as UPC-A (with its check digit) rather than a check-digit-less EAN-13,
    /// which is the convention every scanned competitor and every retail
    /// spreadsheet uses. Anything else falls back to Code 128.
    pub fn resolve(self, value: &str) -> Symbology {
        if self != Self::Auto {
            return self;
        }
        let digits = value.chars().all(|c| c.is_ascii_digit()) && !value.is_empty();
        match (digits, value.len()) {
            (true, 14) => Self::Itf,
            (true, 13) => Self::Ean13,
            (true, 12) => Self::UpcA,
            (true, 8) => Self::Ean8,
            _ => Self::Code128,
        }
    }
}

/// One encoded row: the module vector plus the text actually encoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encoded {
    /// One entry per module; `1` = bar, `0` = space. Quiet zones are NOT included.
    pub modules: Vec<u8>,
    /// Human-readable interpretation — the encoded value incl. any computed check digit.
    pub hri: String,
    /// The symbology actually used (resolved, never `Auto`).
    pub symbology: Symbology,
}

/// GS1 mod-10 check digit. Weights alternate 3,1,3,1… counting from the RIGHT of
/// the supplied data, which is the single rule that covers EAN-13 (12 data
/// digits), UPC-A (11), EAN-8 (7) and ITF-14 (13) without per-symbology casing.
pub fn mod10_check(data: &str) -> Option<u8> {
    let mut sum = 0u32;
    let mut n = 0usize;
    for (i, c) in data.chars().rev().enumerate() {
        let d = c.to_digit(10)?;
        sum += d * if i % 2 == 0 { 3 } else { 1 };
        n += 1;
    }
    if n == 0 {
        return None;
    }
    Some(((10 - (sum % 10)) % 10) as u8)
}

fn digits_only(value: &str) -> Result<String, String> {
    if value.chars().all(|c| c.is_ascii_digit()) {
        Ok(value.to_string())
    } else {
        Err("value must contain digits only".to_string())
    }
}

/// Normalise a retail value to `want` digits, computing the trailing check digit
/// when the caller supplied `want - 1` digits and `auto_check` is on.
fn fit_with_check(value: &str, want: usize, auto_check: bool) -> Result<String, String> {
    let v = digits_only(value)?;
    if v.len() == want {
        let (body, given) = v.split_at(want - 1);
        let expect = mod10_check(body).ok_or("value must contain digits only")?;
        let given: u32 = given.parse().map_err(|_| "value must contain digits only")?;
        if given as u8 != expect {
            return Err(format!(
                "check digit {given} is wrong for {body} — expected {expect}"
            ));
        }
        return Ok(v);
    }
    if v.len() == want - 1 {
        if !auto_check {
            return Err(format!(
                "needs {want} digits (got {}) — turn on the check-digit option to compute the last digit",
                v.len()
            ));
        }
        let c = mod10_check(&v).ok_or("value must contain digits only")?;
        return Ok(format!("{v}{c}"));
    }
    Err(format!(
        "needs {want} digits, or {} without the check digit (got {})",
        want - 1,
        v.len()
    ))
}

/// Code 128 start character: subset C (paired digits, half the width) when the
/// value is an even-length digit run, otherwise subset B (full printable ASCII).
fn code128_with_subset(value: &str) -> Result<Code128, String> {
    let all_digits = !value.is_empty() && value.chars().all(|c| c.is_ascii_digit());
    if let Some(bad) = value.chars().find(|c| !(' '..='~').contains(c)) {
        return Err(format!(
            "Code 128 here covers printable ASCII only — `{bad}` is not encodable"
        ));
    }
    let start = if all_digits && value.len() % 2 == 0 { 'Ć' } else { 'Ɓ' };
    Code128::new(format!("{start}{value}")).map_err(|e| e.to_string())
}

/// Encode one row. `auto_check` controls whether a missing trailing mod-10 check
/// digit is computed for the retail symbologies (EAN-13/EAN-8/UPC-A/ITF-14).
pub fn encode(sym: Symbology, value: &str, auto_check: bool) -> Result<Encoded, String> {
    let sym = sym.resolve(value);
    let (modules, hri) = match sym {
        Symbology::Auto => unreachable!("resolve() never returns Auto"),
        Symbology::Code128 => {
            let hri = value.to_string();
            (code128_with_subset(value)?.encode(), hri)
        }
        Symbology::Code39 => {
            let hri = value.to_ascii_uppercase();
            let bc = Code39::new(&hri).map_err(|e| {
                format!("{e} — Code 39 covers 0-9, A-Z, space and - . $ / + %")
            })?;
            (bc.encode(), hri)
        }
        Symbology::Code93 => {
            let hri = value.to_ascii_uppercase();
            let bc = Code93::new(&hri).map_err(|e| {
                format!("{e} — Code 93 covers 0-9, A-Z, space and - . $ / + %")
            })?;
            (bc.encode(), hri)
        }
        Symbology::Ean13 => {
            let hri = fit_with_check(value, 13, auto_check)?;
            // barcoders takes the first 12 digits and appends the check itself.
            let bc = EAN13::new(&hri).map_err(|e| e.to_string())?;
            (bc.encode(), hri)
        }
        Symbology::UpcA => {
            // A UPC-A 12-digit code is the EAN-13 `0` + the same 12 digits.
            let hri = fit_with_check(value, 12, auto_check)?;
            let bc = UPCA::new(format!("0{hri}")).map_err(|e| e.to_string())?;
            (bc.encode(), hri)
        }
        Symbology::Ean8 => {
            let hri = fit_with_check(value, 8, auto_check)?;
            let bc = EAN8::new(&hri).map_err(|e| e.to_string())?;
            (bc.encode(), hri)
        }
        Symbology::Itf => {
            let v = digits_only(value)?;
            if v.len() < 2 {
                return Err("ITF needs at least 2 digits".to_string());
            }
            // Interleaved 2 of 5 encodes digit PAIRS. An odd-length value gets a
            // mod-10 check digit appended (this is exactly how 13 digits becomes
            // a 14-digit ITF-14); an even-length value is taken as already complete.
            let hri = if v.len() % 2 == 1 {
                if !auto_check {
                    return Err(format!(
                        "ITF needs an even digit count (got {}) — turn on the check-digit option to append one",
                        v.len()
                    ));
                }
                let c = mod10_check(&v).ok_or("value must contain digits only")?;
                format!("{v}{c}")
            } else {
                v
            };
            let bc = TF::interleaved(&hri).map_err(|e| e.to_string())?;
            (bc.encode(), hri)
        }
        Symbology::Codabar => {
            let upper = value.to_ascii_uppercase();
            let starts = upper.starts_with(['A', 'B', 'C', 'D']);
            let ends = upper.ends_with(['A', 'B', 'C', 'D']);
            // Codabar carries its own start/stop letters. Wrap bare values in the
            // conventional A…A pair rather than rejecting them.
            let hri = if starts && ends && upper.len() >= 3 {
                upper
            } else {
                format!("A{upper}A")
            };
            let bc = Codabar::new(&hri).map_err(|e| {
                format!("{e} — Codabar covers 0-9 and - $ : / . +, wrapped in A-D start/stop letters")
            })?;
            (bc.encode(), hri)
        }
    };
    if modules.is_empty() {
        return Err("encoder produced an empty symbol".to_string());
    }
    Ok(Encoded {
        modules,
        hri,
        symbology: sym,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upc_a_check_digit_is_computed() {
        // The canonical GS1 worked example: 03600029145 -> check digit 2.
        let e = encode(Symbology::UpcA, "03600029145", true).unwrap();
        assert_eq!(e.hri, "036000291452");
        assert_eq!(e.symbology, Symbology::UpcA);
        assert!(e.modules.iter().all(|m| *m <= 1));
    }

    #[test]
    fn ean13_round_trips_a_complete_code() {
        let e = encode(Symbology::Ean13, "5901234123457", true).unwrap();
        assert_eq!(e.hri, "5901234123457");
        // EAN-13 is a fixed 95-module symbol.
        assert_eq!(e.modules.len(), 95);
    }

    #[test]
    fn wrong_check_digit_is_rejected() {
        let err = encode(Symbology::Ean13, "5901234123456", true).unwrap_err();
        assert!(err.contains("check digit"), "{err}");
    }

    #[test]
    fn auto_picks_by_shape() {
        assert_eq!(Symbology::Auto.resolve("5901234123457"), Symbology::Ean13);
        assert_eq!(Symbology::Auto.resolve("036000291452"), Symbology::UpcA);
        assert_eq!(Symbology::Auto.resolve("96385074"), Symbology::Ean8);
        assert_eq!(Symbology::Auto.resolve("SKU-1001"), Symbology::Code128);
    }

    #[test]
    fn itf14_appends_the_check_digit() {
        let e = encode(Symbology::Itf, "1234567890123", true).unwrap();
        assert_eq!(e.hri.len(), 14);
        assert_eq!(e.hri, "12345678901231");
    }

    #[test]
    fn code128_subset_c_is_narrower_than_subset_b() {
        let c = encode(Symbology::Code128, "12345678", true).unwrap();
        let b = encode(Symbology::Code128, "1234567A", true).unwrap();
        assert!(
            c.modules.len() < b.modules.len(),
            "digit-only values should use the double-density subset C: {} vs {}",
            c.modules.len(),
            b.modules.len()
        );
    }

    #[test]
    fn codabar_gets_start_stop_letters() {
        let e = encode(Symbology::Codabar, "1234", true).unwrap();
        assert_eq!(e.hri, "A1234A");
    }

    #[test]
    fn code39_uppercases_and_rejects_bad_characters() {
        assert_eq!(encode(Symbology::Code39, "ab-1", true).unwrap().hri, "AB-1");
        assert!(encode(Symbology::Code39, "a=b", true).is_err());
    }

    #[test]
    fn missing_check_digit_needs_the_option() {
        let err = encode(Symbology::Ean13, "590123412345", false).unwrap_err();
        assert!(err.contains("check-digit option"), "{err}");
    }
}
