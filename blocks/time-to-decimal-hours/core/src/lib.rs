//! time-to-decimal-hours core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps.
//!
//! Converts a clock duration (`HH:MM` or `HH:MM:SS`) into decimal hours
//! (e.g. `1:30` → `1.5`) and back (e.g. `1.5` → `1:30`). The conversion is
//! bidirectional: every result reports BOTH the clock form and the decimal
//! form, plus flat totals in minutes and seconds, so the caller never has to
//! call twice. All arithmetic is canonicalised through an integer number of
//! seconds, which keeps every surface deterministic.

use serde::Serialize;

/// A fully-resolved conversion, reported the same way from every surface.
#[derive(Serialize, Debug, PartialEq)]
pub struct Conversion {
    /// The raw input, trimmed.
    pub input: String,
    /// How the input was interpreted: `"clock"` (HH:MM[:SS]) or `"decimal"`.
    pub detected_input: String,
    /// Canonical clock form (`H:MM` when whole-minute, else `H:MM:SS`), signed.
    pub clock: String,
    /// The duration in decimal hours, rounded to 4 decimal places (e.g. 1.5).
    pub decimal_hours: f64,
    /// The duration as a flat number of minutes, rounded to 4 decimal places.
    pub total_minutes: f64,
    /// The duration as a whole number of seconds.
    pub total_seconds: i64,
    /// A human-readable one-line summary.
    pub summary: String,
}

fn round4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

/// Parse a clock duration like `1:30`, `01:30:45`, `-0:30`. Allows an optional
/// leading sign, requires `H:MM`, and accepts an optional `:SS`. Minutes and
/// seconds must be in `0..=59`. Returns total signed seconds.
fn parse_clock(s: &str) -> Result<i64, String> {
    let (sign, body) = match s.strip_prefix('-') {
        Some(rest) => (-1i64, rest.trim()),
        None => (1i64, s.strip_prefix('+').unwrap_or(s).trim()),
    };
    let parts: Vec<&str> = body.split(':').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return Err(format!(
            "'{s}' is not a valid clock duration — use H:MM or H:MM:SS"
        ));
    }
    let parse_u = |p: &str, what: &str| -> Result<i64, String> {
        let p = p.trim();
        if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!("'{s}' has a non-numeric {what} component"));
        }
        p.parse::<i64>()
            .map_err(|_| format!("'{s}' has an out-of-range {what} component"))
    };
    let hours = parse_u(parts[0], "hours")?;
    let minutes = parse_u(parts[1], "minutes")?;
    if minutes > 59 {
        return Err(format!("'{s}' has minutes ({minutes}) outside 0–59"));
    }
    let seconds = if parts.len() == 3 {
        let sec = parse_u(parts[2], "seconds")?;
        if sec > 59 {
            return Err(format!("'{s}' has seconds ({sec}) outside 0–59"));
        }
        sec
    } else {
        0
    };
    Ok(sign * (hours * 3600 + minutes * 60 + seconds))
}

/// Parse decimal hours like `1.5`, `-0.25`, `2`. Returns total signed seconds,
/// rounded to the nearest whole second.
fn parse_decimal(s: &str) -> Result<i64, String> {
    let v: f64 = s
        .trim()
        .parse()
        .map_err(|_| format!("'{s}' is not a valid number of decimal hours"))?;
    if !v.is_finite() {
        return Err(format!("'{s}' is not a finite number"));
    }
    Ok((v * 3600.0).round() as i64)
}

/// Format whole signed seconds as a canonical clock string. Uses `H:MM` when the
/// seconds component is zero, otherwise `H:MM:SS`.
fn seconds_to_clock(total: i64) -> String {
    let sign = if total < 0 { "-" } else { "" };
    let t = total.unsigned_abs();
    let h = t / 3600;
    let m = (t % 3600) / 60;
    let s = t % 60;
    if s == 0 {
        format!("{sign}{h}:{m:02}")
    } else {
        format!("{sign}{h}:{m:02}:{s:02}")
    }
}

/// Convert `value` to a fully-resolved [`Conversion`]. `mode` selects how the
/// input is interpreted:
/// - `"auto"` (default): treat input as a clock duration if it contains `:`,
///   otherwise as decimal hours.
/// - `"from-clock"`: always parse the input as a clock duration.
/// - `"from-decimal"`: always parse the input as decimal hours.
pub fn convert(value: &str, mode: &str) -> Result<Conversion, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("no value provided".into());
    }
    let (detected, total_seconds) = match mode.trim() {
        "" | "auto" => {
            if trimmed.contains(':') {
                ("clock", parse_clock(trimmed)?)
            } else {
                ("decimal", parse_decimal(trimmed)?)
            }
        }
        "from-clock" => ("clock", parse_clock(trimmed)?),
        "from-decimal" => ("decimal", parse_decimal(trimmed)?),
        other => {
            return Err(format!(
                "unknown mode '{other}' — use auto, from-clock, or from-decimal"
            ))
        }
    };

    let decimal_hours = round4(total_seconds as f64 / 3600.0);
    let total_minutes = round4(total_seconds as f64 / 60.0);
    let clock = seconds_to_clock(total_seconds);
    let summary =
        format!("{clock} = {decimal_hours} decimal hours ({total_minutes} minutes)");

    Ok(Conversion {
        input: trimmed.to_string(),
        detected_input: detected.to_string(),
        clock,
        decimal_hours,
        total_minutes,
        total_seconds,
        summary,
    })
}

/// Convert and return a pretty-printed JSON object (used by the web page).
pub fn convert_json(value: &str, mode: &str) -> Result<String, String> {
    let c = convert(value, mode)?;
    serde_json::to_string_pretty(&c).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_to_decimal_basic() {
        let c = convert("1:30", "auto").unwrap();
        assert_eq!(c.detected_input, "clock");
        assert_eq!(c.decimal_hours, 1.5);
        assert_eq!(c.total_minutes, 90.0);
        assert_eq!(c.total_seconds, 5400);
        assert_eq!(c.clock, "1:30");
    }

    #[test]
    fn decimal_to_clock_basic() {
        let c = convert("1.5", "auto").unwrap();
        assert_eq!(c.detected_input, "decimal");
        assert_eq!(c.clock, "1:30");
        assert_eq!(c.total_seconds, 5400);
    }

    #[test]
    fn clock_with_seconds() {
        let c = convert("2:15:18", "auto").unwrap();
        assert_eq!(c.total_seconds, 2 * 3600 + 15 * 60 + 18);
        assert_eq!(c.clock, "2:15:18");
        assert_eq!(c.decimal_hours, round4(8118.0 / 3600.0));
    }

    #[test]
    fn decimal_rounds_to_nearest_second() {
        // 1.3333 h ≈ 1:20:00 (4800 s); round(1.3333*3600) = 4800.
        let c = convert("1.3333", "from-decimal").unwrap();
        assert_eq!(c.total_seconds, 4800);
        assert_eq!(c.clock, "1:20");
    }

    #[test]
    fn one_third_hour_clock() {
        let c = convert("0:20", "auto").unwrap();
        assert_eq!(c.total_seconds, 1200);
        assert_eq!(c.decimal_hours, 0.3333);
    }

    #[test]
    fn whole_number_decimal() {
        let c = convert("8", "from-decimal").unwrap();
        assert_eq!(c.clock, "8:00");
        assert_eq!(c.decimal_hours, 8.0);
        assert_eq!(c.total_seconds, 28800);
    }

    #[test]
    fn negative_clock() {
        let c = convert("-1:30", "auto").unwrap();
        assert_eq!(c.decimal_hours, -1.5);
        assert_eq!(c.total_seconds, -5400);
        assert_eq!(c.clock, "-1:30");
    }

    #[test]
    fn negative_decimal() {
        let c = convert("-0.25", "auto").unwrap();
        assert_eq!(c.clock, "-0:15");
        assert_eq!(c.total_seconds, -900);
    }

    #[test]
    fn zero() {
        let c = convert("0:00", "auto").unwrap();
        assert_eq!(c.decimal_hours, 0.0);
        assert_eq!(c.clock, "0:00");
        assert_eq!(c.total_seconds, 0);
    }

    #[test]
    fn forced_mode_overrides_autodetect() {
        // A plain number forced as clock has no colon → error.
        assert!(convert("1.5", "from-clock").is_err());
        // A colon value forced as decimal → error.
        assert!(convert("1:30", "from-decimal").is_err());
    }

    #[test]
    fn rejects_bad_minutes() {
        assert!(convert("1:90", "auto").is_err());
        assert!(convert("1:30:90", "auto").is_err());
    }

    #[test]
    fn rejects_garbage() {
        assert!(convert("abc", "auto").is_err());
        assert!(convert("", "auto").is_err());
        assert!(convert("1:2:3:4", "auto").is_err());
    }

    #[test]
    fn large_duration() {
        let c = convert("40:30", "auto").unwrap();
        assert_eq!(c.decimal_hours, 40.5);
        assert_eq!(c.total_seconds, 145800);
    }

    #[test]
    fn json_shape() {
        let j = convert_json("1:30", "auto").unwrap();
        assert!(j.contains("\"decimal_hours\": 1.5"));
        assert!(j.contains("\"clock\": \"1:30\""));
        assert!(j.contains("\"detected_input\": \"clock\""));
    }
}
