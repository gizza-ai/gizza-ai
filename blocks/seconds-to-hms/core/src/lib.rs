//! seconds-to-hms core — convert a number of seconds into a clock-style
//! duration string (HH:MM:SS, D:HH:MM:SS, a shortest "auto" form, ISO-8601, or
//! human-readable words). No wafer/wasm-bindgen deps. Shared by the chat skill
//! block and the web page.

/// Maximum fractional-second digits the formatter will render. `decimals` above
/// this is clamped down to it.
pub const MAX_DECIMALS: u32 = 9;

/// Output layout.
#[derive(Clone, Copy)]
enum Format {
    /// `HH:MM:SS` — the hours field accumulates beyond 24 (e.g. `25:01:01`).
    Hms,
    /// `D:HH:MM:SS` — always splits out a days field (days are NOT zero-padded).
    Dhms,
    /// Shortest form — drops a leading zero days field and a leading zero hours
    /// field (`MM:SS`, `HH:MM:SS`, or `D:HH:MM:SS`). Minutes and seconds always show.
    Auto,
    /// ISO-8601 duration, e.g. `PT1H30M20S` / `P1DT1H1M1S` (zero → `PT0S`).
    Iso,
    /// Human-readable, e.g. `1 hour, 30 minutes, 20 seconds` (zero → `0 seconds`).
    Words,
}

impl Format {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "hms" => Ok(Format::Hms),
            "dhms" => Ok(Format::Dhms),
            "auto" => Ok(Format::Auto),
            "iso" => Ok(Format::Iso),
            "words" => Ok(Format::Words),
            other => Err(format!(
                "invalid format {other:?}: expected \"hms\", \"dhms\", \"auto\", \"iso\", or \"words\""
            )),
        }
    }
}

/// The duration broken into whole day/hour/minute/second components plus the
/// fractional-second units (in `10^decimals` ticks).
struct Parts {
    days: u64,
    hours: u64,
    minutes: u64,
    secs: u64,
    frac_units: u64,
    decimals: u32,
}

impl Parts {
    /// Zero-padded `SS` (with a `.frac` suffix when `decimals > 0`) for clock formats.
    fn sec_field(&self) -> String {
        if self.decimals > 0 {
            format!(
                "{:02}.{:0width$}",
                self.secs,
                self.frac_units,
                width = self.decimals as usize
            )
        } else {
            format!("{:02}", self.secs)
        }
    }

    /// Un-padded seconds (with a `.frac` suffix when `decimals > 0`) for ISO/words.
    fn sec_value(&self) -> String {
        if self.decimals > 0 {
            format!(
                "{}.{:0width$}",
                self.secs,
                self.frac_units,
                width = self.decimals as usize
            )
        } else {
            self.secs.to_string()
        }
    }
}

/// Format `seconds` as a duration string.
///
/// - `seconds`: total seconds; may be fractional or negative (a negative input
///   yields a `-`-prefixed result). Must be a finite number.
/// - `format` (`"hms"` | `"dhms"` | `"auto"` | `"iso"` | `"words"`, blank →
///   `"hms"`): output layout, see [`Format`].
/// - `decimals`: fractional-second digits to append (e.g. `90.5` with
///   `decimals = 1` → `…30.5`). Clamped to `0..=MAX_DECIMALS`; `0` → no decimals.
///
/// Returns `Err` on a non-finite `seconds` or an unrecognised `format`.
pub fn to_hms(seconds: f64, format: &str, decimals: u32) -> Result<String, String> {
    if !seconds.is_finite() {
        return Err(format!("seconds must be a finite number, got {seconds}"));
    }
    let fmt = Format::parse(format)?;
    let decimals = decimals.min(MAX_DECIMALS);

    // -0.0 is not "negative" for display purposes.
    let neg = seconds.is_sign_negative() && seconds != 0.0;
    let total = seconds.abs();

    // Round to the requested precision first so the integer split and the
    // fractional suffix agree (and so e.g. 59.999 rolls over to 00:01:00).
    let scale = 10f64.powi(decimals as i32);
    let rounded = (total * scale).round() / scale;
    let whole = rounded.floor() as u64;
    let frac_units = ((rounded - whole as f64) * scale).round() as u64;

    let p = Parts {
        days: whole / 86_400,
        hours: (whole % 86_400) / 3_600,
        minutes: (whole % 3_600) / 60,
        secs: whole % 60,
        frac_units,
        decimals,
    };

    let body = match fmt {
        Format::Hms => {
            // Days roll into the hours field, which may exceed two digits.
            let total_hours = whole / 3_600;
            format!("{:02}:{:02}:{}", total_hours, p.minutes, p.sec_field())
        }
        Format::Dhms => format!(
            "{}:{:02}:{:02}:{}",
            p.days,
            p.hours,
            p.minutes,
            p.sec_field()
        ),
        Format::Auto => {
            if p.days > 0 {
                format!("{}:{:02}:{:02}:{}", p.days, p.hours, p.minutes, p.sec_field())
            } else if p.hours > 0 {
                format!("{:02}:{:02}:{}", p.hours, p.minutes, p.sec_field())
            } else {
                format!("{:02}:{}", p.minutes, p.sec_field())
            }
        }
        Format::Iso => iso(&p),
        Format::Words => words(&p),
    };

    Ok(if neg { format!("-{body}") } else { body })
}

/// ISO-8601 duration: `P[nD]T[nH][nM][nS]`, omitting zero components. A zero
/// duration is `PT0S`. Fractional seconds use a decimal point (`PT1M30.5S`).
fn iso(p: &Parts) -> String {
    let mut s = String::from("P");
    if p.days > 0 {
        s.push_str(&format!("{}D", p.days));
    }
    let has_secs = p.secs > 0 || p.frac_units > 0;
    if p.hours > 0 || p.minutes > 0 || has_secs {
        s.push('T');
        if p.hours > 0 {
            s.push_str(&format!("{}H", p.hours));
        }
        if p.minutes > 0 {
            s.push_str(&format!("{}M", p.minutes));
        }
        if has_secs {
            s.push_str(&format!("{}S", p.sec_value()));
        }
    }
    if s == "P" {
        s.push_str("T0S");
    }
    s
}

/// Human-readable list, e.g. `1 day, 2 hours, 3 seconds`. Zero components are
/// dropped; an all-zero duration is `0 seconds`. Units are singular when their
/// value is exactly 1.
fn words(p: &Parts) -> String {
    fn unit(value: u64, name: &str) -> String {
        if value == 1 {
            format!("1 {name}")
        } else {
            format!("{value} {name}s")
        }
    }
    let mut parts = Vec::new();
    if p.days > 0 {
        parts.push(unit(p.days, "day"));
    }
    if p.hours > 0 {
        parts.push(unit(p.hours, "hour"));
    }
    if p.minutes > 0 {
        parts.push(unit(p.minutes, "minute"));
    }
    // Always show seconds when there's a fraction, a non-zero second count, or
    // nothing else to show (so a zero/sub-second duration still renders).
    if p.secs > 0 || p.frac_units > 0 || parts.is_empty() {
        let singular = p.secs == 1 && p.frac_units == 0;
        let name = if singular { "second" } else { "seconds" };
        parts.push(format!("{} {name}", p.sec_value()));
    }
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hms_basic() {
        assert_eq!(to_hms(0.0, "hms", 0).unwrap(), "00:00:00");
        assert_eq!(to_hms(61.0, "hms", 0).unwrap(), "00:01:01");
        assert_eq!(to_hms(3661.0, "hms", 0).unwrap(), "01:01:01");
    }

    #[test]
    fn hms_accumulates_hours_past_a_day() {
        // 25h 1m 1s — days roll into the hours field in hms mode.
        assert_eq!(to_hms(90_061.0, "hms", 0).unwrap(), "25:01:01");
    }

    #[test]
    fn dhms_splits_days() {
        assert_eq!(to_hms(90_061.0, "dhms", 0).unwrap(), "1:01:01:01");
        // Days field is shown even when zero.
        assert_eq!(to_hms(61.0, "dhms", 0).unwrap(), "0:00:01:01");
    }

    #[test]
    fn auto_drops_leading_zero_units() {
        assert_eq!(to_hms(5.0, "auto", 0).unwrap(), "00:05");
        assert_eq!(to_hms(61.0, "auto", 0).unwrap(), "01:01");
        assert_eq!(to_hms(3661.0, "auto", 0).unwrap(), "01:01:01");
        assert_eq!(to_hms(90_061.0, "auto", 0).unwrap(), "1:01:01:01");
    }

    #[test]
    fn iso_8601() {
        assert_eq!(to_hms(0.0, "iso", 0).unwrap(), "PT0S");
        assert_eq!(to_hms(20.0, "iso", 0).unwrap(), "PT20S");
        assert_eq!(to_hms(3661.0, "iso", 0).unwrap(), "PT1H1M1S");
        assert_eq!(to_hms(90_061.0, "iso", 0).unwrap(), "P1DT1H1M1S");
        assert_eq!(to_hms(90.5, "iso", 1).unwrap(), "PT1M30.5S");
    }

    #[test]
    fn words_human_readable() {
        assert_eq!(to_hms(0.0, "words", 0).unwrap(), "0 seconds");
        assert_eq!(to_hms(1.0, "words", 0).unwrap(), "1 second");
        assert_eq!(to_hms(3661.0, "words", 0).unwrap(), "1 hour, 1 minute, 1 second");
        assert_eq!(
            to_hms(90_122.0, "words", 0).unwrap(),
            "1 day, 1 hour, 2 minutes, 2 seconds"
        );
        assert_eq!(to_hms(90.5, "words", 1).unwrap(), "1 minute, 30.5 seconds");
    }

    #[test]
    fn fractional_seconds() {
        assert_eq!(to_hms(90.5, "hms", 1).unwrap(), "00:01:30.5");
        assert_eq!(to_hms(90.25, "auto", 2).unwrap(), "01:30.25");
        // decimals above the cap clamp to MAX_DECIMALS rather than panicking.
        assert!(to_hms(1.0, "hms", 99).is_ok());
    }

    #[test]
    fn rounding_rolls_over() {
        // 59.999s rounded to whole seconds becomes a full minute.
        assert_eq!(to_hms(59.999, "hms", 0).unwrap(), "00:01:00");
    }

    #[test]
    fn negative_is_prefixed() {
        assert_eq!(to_hms(-90.0, "auto", 0).unwrap(), "-01:30");
        assert_eq!(to_hms(-3661.0, "hms", 0).unwrap(), "-01:01:01");
        assert_eq!(to_hms(-3661.0, "iso", 0).unwrap(), "-PT1H1M1S");
    }

    #[test]
    fn rejects_non_finite() {
        assert!(to_hms(f64::NAN, "hms", 0).unwrap_err().contains("finite"));
        assert!(to_hms(f64::INFINITY, "hms", 0).unwrap_err().contains("finite"));
    }

    #[test]
    fn rejects_unknown_format() {
        let err = to_hms(1.0, "stopwatch", 0).unwrap_err();
        assert!(err.contains("invalid format"), "got: {err}");
    }
}
