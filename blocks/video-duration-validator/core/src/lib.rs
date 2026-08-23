//! gizza-ai/video-duration-validator core — check a video's container duration
//! against a target length and report PASS/FAIL.
//!
//! The duration is read from the container header with the pure-Rust
//! `symphonia` demuxers (via the media-info block's core) — no decoding, no
//! ffmpeg, no I/O — so the identical check runs in the chat block, the CLI, and
//! the browser page.
//!
//! The comparison itself is split out into [`check`], which takes a plain
//! `f64` duration. That keeps the rule table testable without media bytes and
//! lets a caller who already knows the duration reuse it.

use serde::Serialize;

/// How the actual duration is compared with the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// PASS when the duration sits inside `target ± tolerance` (default).
    Within,
    /// Upper bound: PASS when the duration is at most `target + tolerance`.
    Max,
    /// Lower bound: PASS when the duration is at least `target - tolerance`.
    Min,
}

impl Mode {
    /// Canonical wire name, as it appears in the schema and the JSON output.
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Within => "within",
            Mode::Max => "max",
            Mode::Min => "min",
        }
    }

    /// Parse the enum value, with a message naming the accepted values.
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim() {
            "within" => Ok(Mode::Within),
            "max" => Ok(Mode::Max),
            "min" => Ok(Mode::Min),
            other => Err(format!(
                "unknown mode {other:?}; use one of within, max, min"
            )),
        }
    }
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Within
    }
}

/// Largest accepted target/tolerance, in seconds (24 hours). Beyond this the
/// input is almost certainly milliseconds or a typo rather than a clip length.
pub const MAX_SECONDS: f64 = 86_400.0;

/// The validation rule to apply.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    /// Target length in seconds. Required — there is no sensible default.
    pub target_seconds: f64,
    /// Allowed slack around the target, in seconds.
    pub tolerance_seconds: f64,
    pub mode: Mode,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            target_seconds: 0.0,
            tolerance_seconds: 0.5,
            mode: Mode::Within,
        }
    }
}

impl Options {
    /// Reject out-of-range or non-finite numbers BEFORE any file is fetched, so
    /// a bad rule fails fast and cheaply.
    pub fn validate(&self) -> Result<(), String> {
        if !self.target_seconds.is_finite() {
            return Err("target_seconds must be a finite number of seconds".into());
        }
        if !self.tolerance_seconds.is_finite() {
            return Err("tolerance_seconds must be a finite number of seconds".into());
        }
        if self.target_seconds <= 0.0 {
            return Err(format!(
                "target_seconds must be greater than 0, got {}",
                trim_num(self.target_seconds)
            ));
        }
        if self.target_seconds > MAX_SECONDS {
            return Err(format!(
                "target_seconds must not exceed {} (24 hours), got {}",
                trim_num(MAX_SECONDS),
                trim_num(self.target_seconds)
            ));
        }
        if self.tolerance_seconds < 0.0 {
            return Err(format!(
                "tolerance_seconds must not be negative, got {}",
                trim_num(self.tolerance_seconds)
            ));
        }
        if self.tolerance_seconds > MAX_SECONDS {
            return Err(format!(
                "tolerance_seconds must not exceed {} (24 hours), got {}",
                trim_num(MAX_SECONDS),
                trim_num(self.tolerance_seconds)
            ));
        }
        Ok(())
    }
}

/// The verdict for one clip.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// "PASS" or "FAIL" — the single field a script should branch on.
    pub status: &'static str,
    /// True mirror of `status`, for callers that prefer a boolean.
    pub pass: bool,
    /// Why it passed or failed: "ok", "too_long", or "too_short".
    pub reason: &'static str,
    /// The comparison that was applied.
    pub mode: &'static str,
    /// Container duration in seconds, rounded to milliseconds.
    pub actual_seconds: f64,
    /// The same duration as `h:mm:ss.mmm` / `m:ss.mmm`.
    pub actual_duration: String,
    pub target_seconds: f64,
    /// The target as `h:mm:ss.mmm` / `m:ss.mmm`.
    pub target_duration: String,
    pub tolerance_seconds: f64,
    /// `actual - target`: positive means too long, negative means too short.
    pub delta_seconds: f64,
    /// How far outside the allowed window it landed; 0 on a PASS.
    pub overshoot_seconds: f64,
    /// Lowest accepted duration, omitted when the rule has no lower bound.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_min_seconds: Option<f64>,
    /// Highest accepted duration, omitted when the rule has no upper bound.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_max_seconds: Option<f64>,
    /// Friendly container label, e.g. "MP4 / M4A (ISO BMFF)".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
    /// One-line human summary of the verdict.
    pub summary: String,
}

/// Apply the rule to a duration that is already known, in seconds.
///
/// `container` is only used to label the report. Errors when `duration_seconds`
/// is not a usable number or the options are out of range.
pub fn check(
    duration_seconds: f64,
    opts: &Options,
    container: Option<&str>,
) -> Result<Report, String> {
    opts.validate()?;
    if !duration_seconds.is_finite() || duration_seconds < 0.0 {
        return Err(
            "the container does not record a usable duration; remux the file to rebuild its \
             header (see the video-duration-fix-remux tool) and try again"
                .into(),
        );
    }

    let actual = round3(duration_seconds);
    let target = round3(opts.target_seconds);
    let tolerance = round3(opts.tolerance_seconds);
    let delta = round3(actual - target);

    // Each mode is a window; `None` means that side is unbounded.
    let (min, max) = match opts.mode {
        Mode::Within => (Some(round3(target - tolerance)), Some(round3(target + tolerance))),
        Mode::Max => (None, Some(round3(target + tolerance))),
        Mode::Min => (Some(round3(target - tolerance)), None),
    };

    let over = max.map_or(0.0, |m| round3(actual - m)).max(0.0);
    let under = min.map_or(0.0, |m| round3(m - actual)).max(0.0);

    let (pass, reason, overshoot) = if over > 0.0 {
        (false, "too_long", round3(over))
    } else if under > 0.0 {
        (false, "too_short", round3(under))
    } else {
        (true, "ok", 0.0)
    };

    let summary = summary(pass, reason, opts.mode, actual, target, tolerance, overshoot);

    Ok(Report {
        status: if pass { "PASS" } else { "FAIL" },
        pass,
        reason,
        mode: opts.mode.as_str(),
        actual_seconds: actual,
        actual_duration: human_duration(actual),
        target_seconds: target,
        target_duration: human_duration(target),
        tolerance_seconds: tolerance,
        delta_seconds: delta,
        overshoot_seconds: overshoot,
        allowed_min_seconds: min,
        allowed_max_seconds: max,
        container: container.map(|c| c.to_string()),
        summary,
    })
}

/// Read `bytes` as a media container and apply the rule to its duration.
pub fn validate(bytes: &[u8], opts: &Options) -> Result<Report, String> {
    // Validate the rule first: a bad target should not be masked by a demux error.
    opts.validate()?;
    let info = gizza_ai_media_info_core::info(bytes)?;
    let duration = info.duration_seconds.ok_or_else(|| {
        format!(
            "no duration is recorded in this {} file; the container header is missing or broken \
             — remux it (see the video-duration-fix-remux tool) and try again",
            info.container
        )
    })?;
    check(duration, opts, Some(&info.container))
}

/// [`validate`] with the report serialized as pretty JSON.
pub fn validate_json(bytes: &[u8], opts: &Options) -> Result<String, String> {
    let report = validate(bytes, opts)?;
    serde_json::to_string_pretty(&report).map_err(|e| format!("serialize report: {e}"))
}

fn summary(
    pass: bool,
    reason: &str,
    mode: Mode,
    actual: f64,
    target: f64,
    tolerance: f64,
    overshoot: f64,
) -> String {
    let rule = match mode {
        Mode::Within => format!(
            "{}s ± {}s",
            trim_num(target),
            trim_num(tolerance)
        ),
        Mode::Max => format!("at most {}s (+{}s tolerance)", trim_num(target), trim_num(tolerance)),
        Mode::Min => format!("at least {}s (-{}s tolerance)", trim_num(target), trim_num(tolerance)),
    };
    if pass {
        return format!(
            "PASS — {} ({}s) meets the rule {}.",
            human_duration(actual),
            trim_num(actual),
            rule
        );
    }
    let direction = if reason == "too_long" { "long" } else { "short" };
    format!(
        "FAIL — {} ({}s) is {}s too {} for the rule {}.",
        human_duration(actual),
        trim_num(actual),
        trim_num(overshoot),
        direction,
        rule
    )
}

/// `h:mm:ss.mmm` past an hour, else `m:ss.mmm`.
pub fn human_duration(seconds: f64) -> String {
    let total = seconds.max(0.0);
    let h = (total / 3600.0).floor() as u64;
    let m = ((total - h as f64 * 3600.0) / 60.0).floor() as u64;
    let s = total - h as f64 * 3600.0 - m as f64 * 60.0;
    if h > 0 {
        format!("{h}:{m:02}:{s:06.3}")
    } else {
        format!("{m}:{s:06.3}")
    }
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// Render a number without trailing zeros: `30` not `30.000`, `0.5` not `0.500`.
fn trim_num(v: f64) -> String {
    let s = format!("{:.3}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(target: f64, tol: f64, mode: Mode) -> Options {
        Options {
            target_seconds: target,
            tolerance_seconds: tol,
            mode,
        }
    }

    #[test]
    fn defaults_are_within_with_half_a_second() {
        let d = Options::default();
        assert_eq!(d.tolerance_seconds, 0.5);
        assert_eq!(d.mode, Mode::Within);
        assert_eq!(Mode::default(), Mode::Within);
    }

    #[test]
    fn within_passes_inside_the_window() {
        let r = check(30.2, &opts(30.0, 0.5, Mode::Within), None).unwrap();
        assert_eq!(r.status, "PASS");
        assert!(r.pass);
        assert_eq!(r.reason, "ok");
        assert_eq!(r.delta_seconds, 0.2);
        assert_eq!(r.overshoot_seconds, 0.0);
        assert_eq!(r.allowed_min_seconds, Some(29.5));
        assert_eq!(r.allowed_max_seconds, Some(30.5));
        assert_eq!(r.actual_duration, "0:30.200");
        assert!(r.summary.starts_with("PASS — 0:30.200 (30.2s)"), "{}", r.summary);
    }

    #[test]
    fn within_flags_a_clip_that_is_too_long() {
        let r = check(31.25, &opts(30.0, 0.5, Mode::Within), Some("MP4 / M4A (ISO BMFF)")).unwrap();
        assert_eq!(r.status, "FAIL");
        assert_eq!(r.reason, "too_long");
        assert_eq!(r.delta_seconds, 1.25);
        assert_eq!(r.overshoot_seconds, 0.75);
        assert_eq!(r.container.as_deref(), Some("MP4 / M4A (ISO BMFF)"));
        assert!(r.summary.contains("0.75s too long"), "{}", r.summary);
    }

    #[test]
    fn within_flags_a_clip_that_is_too_short() {
        let r = check(28.0, &opts(30.0, 0.5, Mode::Within), None).unwrap();
        assert_eq!(r.status, "FAIL");
        assert_eq!(r.reason, "too_short");
        assert_eq!(r.delta_seconds, -2.0);
        assert_eq!(r.overshoot_seconds, 1.5);
        assert!(r.summary.contains("1.5s too short"), "{}", r.summary);
    }

    #[test]
    fn boundaries_are_inclusive() {
        assert_eq!(check(30.5, &opts(30.0, 0.5, Mode::Within), None).unwrap().status, "PASS");
        assert_eq!(check(29.5, &opts(30.0, 0.5, Mode::Within), None).unwrap().status, "PASS");
        assert_eq!(check(30.501, &opts(30.0, 0.5, Mode::Within), None).unwrap().status, "FAIL");
        assert_eq!(check(29.499, &opts(30.0, 0.5, Mode::Within), None).unwrap().status, "FAIL");
    }

    #[test]
    fn max_mode_only_bounds_from_above() {
        let short = check(4.0, &opts(60.0, 0.0, Mode::Max), None).unwrap();
        assert_eq!(short.status, "PASS");
        assert_eq!(short.allowed_min_seconds, None);
        assert_eq!(short.allowed_max_seconds, Some(60.0));

        let long = check(62.5, &opts(60.0, 0.5, Mode::Max), None).unwrap();
        assert_eq!(long.status, "FAIL");
        assert_eq!(long.reason, "too_long");
        assert_eq!(long.overshoot_seconds, 2.0);
        assert!(long.summary.contains("at most 60s"), "{}", long.summary);
    }

    #[test]
    fn min_mode_only_bounds_from_below() {
        let long = check(600.0, &opts(15.0, 0.0, Mode::Min), None).unwrap();
        assert_eq!(long.status, "PASS");
        assert_eq!(long.allowed_max_seconds, None);
        assert_eq!(long.allowed_min_seconds, Some(15.0));

        let short = check(12.0, &opts(15.0, 0.5, Mode::Min), None).unwrap();
        assert_eq!(short.status, "FAIL");
        assert_eq!(short.reason, "too_short");
        assert_eq!(short.overshoot_seconds, 2.5);
        assert!(short.summary.contains("at least 15s"), "{}", short.summary);
    }

    #[test]
    fn zero_tolerance_demands_an_exact_match() {
        assert_eq!(check(10.0, &opts(10.0, 0.0, Mode::Within), None).unwrap().status, "PASS");
        assert_eq!(check(10.001, &opts(10.0, 0.0, Mode::Within), None).unwrap().status, "FAIL");
    }

    #[test]
    fn mode_parses_and_rejects_unknown_values() {
        assert_eq!(Mode::parse("within").unwrap(), Mode::Within);
        assert_eq!(Mode::parse(" max ").unwrap(), Mode::Max);
        assert_eq!(Mode::parse("min").unwrap(), Mode::Min);
        let e = Mode::parse("exactly").unwrap_err();
        assert!(e.contains("within, max, min"), "{e}");
    }

    #[test]
    fn out_of_range_rules_are_rejected() {
        assert!(check(10.0, &opts(0.0, 0.5, Mode::Within), None)
            .unwrap_err()
            .contains("target_seconds must be greater than 0"));
        assert!(check(10.0, &opts(-5.0, 0.5, Mode::Within), None)
            .unwrap_err()
            .contains("greater than 0"));
        assert!(check(10.0, &opts(90_000.0, 0.5, Mode::Within), None)
            .unwrap_err()
            .contains("24 hours"));
        assert!(check(10.0, &opts(30.0, -1.0, Mode::Within), None)
            .unwrap_err()
            .contains("must not be negative"));
        assert!(check(10.0, &opts(30.0, 90_000.0, Mode::Within), None)
            .unwrap_err()
            .contains("24 hours"));
        assert!(check(10.0, &opts(f64::NAN, 0.5, Mode::Within), None)
            .unwrap_err()
            .contains("finite"));
        assert!(check(10.0, &opts(30.0, f64::INFINITY, Mode::Within), None)
            .unwrap_err()
            .contains("finite"));
    }

    #[test]
    fn an_unusable_duration_is_an_error_not_a_fail() {
        let e = check(f64::INFINITY, &opts(30.0, 0.5, Mode::Within), None).unwrap_err();
        assert!(e.contains("does not record a usable duration"), "{e}");
        assert!(check(-1.0, &opts(30.0, 0.5, Mode::Within), None).is_err());
    }

    #[test]
    fn validate_rejects_empty_and_non_media_bytes() {
        let o = opts(30.0, 0.5, Mode::Within);
        assert!(validate(&[], &o).is_err());
        assert!(validate(b"this is plain text, not a video container", &o).is_err());
    }

    #[test]
    fn validate_checks_the_rule_before_reading_the_file() {
        // Garbage bytes AND a bad target: the target error must win, so a
        // scripted caller learns the rule is wrong without a fetch.
        let e = validate(b"garbage", &opts(-1.0, 0.5, Mode::Within)).unwrap_err();
        assert!(e.contains("target_seconds"), "{e}");
    }

    #[test]
    fn validate_reads_a_real_container_duration() {
        // 1.0 s of 8-bit mono PCM at 8 kHz: 44-byte WAV header + 8000 samples.
        let wav = wav_1s();
        let pass = validate(&wav, &opts(1.0, 0.1, Mode::Within)).unwrap();
        assert_eq!(pass.status, "PASS");
        assert_eq!(pass.actual_seconds, 1.0);
        assert_eq!(pass.container.as_deref(), Some("WAVE (RIFF)"));

        let fail = validate(&wav, &opts(30.0, 0.5, Mode::Within)).unwrap();
        assert_eq!(fail.status, "FAIL");
        assert_eq!(fail.reason, "too_short");
        assert_eq!(fail.delta_seconds, -29.0);
    }

    #[test]
    fn validate_json_emits_the_documented_fields() {
        let json = validate_json(&wav_1s(), &opts(1.0, 0.1, Mode::Within)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["status"], "PASS");
        assert_eq!(v["mode"], "within");
        assert_eq!(v["actual_seconds"], 1.0);
        assert_eq!(v["target_seconds"], 1.0);
        assert_eq!(v["tolerance_seconds"], 0.1);
        assert_eq!(v["delta_seconds"], 0.0);
        assert_eq!(v["actual_duration"], "0:01.000");
    }

    #[test]
    fn human_duration_formats_minutes_and_hours() {
        assert_eq!(human_duration(83.45), "1:23.450");
        assert_eq!(human_duration(0.0), "0:00.000");
        assert!(human_duration(3661.0).starts_with("1:01:01"));
    }

    #[test]
    fn trim_num_drops_trailing_zeros() {
        assert_eq!(trim_num(30.0), "30");
        assert_eq!(trim_num(0.5), "0.5");
        assert_eq!(trim_num(1.25), "1.25");
        assert_eq!(trim_num(0.0), "0");
    }

    /// Minimal valid 1-second 8 kHz 8-bit mono PCM WAV.
    fn wav_1s() -> Vec<u8> {
        let sample_rate: u32 = 8000;
        let data = vec![128u8; 8000];
        let mut w = Vec::new();
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&((36 + data.len()) as u32).to_le_bytes());
        w.extend_from_slice(b"WAVE");
        w.extend_from_slice(b"fmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes()); // PCM
        w.extend_from_slice(&1u16.to_le_bytes()); // mono
        w.extend_from_slice(&sample_rate.to_le_bytes());
        w.extend_from_slice(&sample_rate.to_le_bytes()); // byte rate
        w.extend_from_slice(&1u16.to_le_bytes()); // block align
        w.extend_from_slice(&8u16.to_le_bytes()); // bits per sample
        w.extend_from_slice(b"data");
        w.extend_from_slice(&(data.len() as u32).to_le_bytes());
        w.extend_from_slice(&data);
        w
    }
}
