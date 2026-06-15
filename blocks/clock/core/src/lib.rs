//! gizza-ai/clock core — UTC RFC-3339 formatting shared by the chat skill block
//! and the standalone web page. Pure; no IO, no wafer/wasm-bindgen. Keeps
//! `chrono` so the formatted string byte-matches the existing skill contract.

use chrono::{DateTime, Utc};

/// Format an already-resolved UTC datetime as RFC-3339, e.g.
/// "2026-04-20T12:00:00+00:00". This is the exact form the chat skill emits.
pub fn format_rfc3339(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339()
}

/// Format a Unix timestamp (seconds) as UTC RFC-3339. Used by the web page,
/// which supplies `Date.now()/1000` from JS. Returns an empty string only for
/// timestamps outside chrono's representable range (not reachable in practice).
pub fn format_secs(secs: i64) -> String {
    DateTime::<Utc>::from_timestamp(secs, 0)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone as _;

    #[test]
    fn format_rfc3339_matches_skill_contract() {
        let dt = Utc.with_ymd_and_hms(2026, 4, 20, 12, 0, 0).unwrap();
        assert_eq!(format_rfc3339(dt), "2026-04-20T12:00:00+00:00");
    }

    #[test]
    fn format_secs_epoch_and_known() {
        assert_eq!(format_secs(0), "1970-01-01T00:00:00+00:00");
        // 1_700_000_000 = 2023-11-14T22:13:20Z
        assert_eq!(format_secs(1_700_000_000), "2023-11-14T22:13:20+00:00");
    }
}
