//! evtx-parser core — pure-Rust Windows Event Log (`.evtx`) reader.
//!
//! Wraps the `evtx` crate (binary EVTX chunk parser) to turn a raw `.evtx`
//! buffer into LLM-friendly structured JSON: a flat list of records, each with
//! its record id, timestamp, event id, provider, level, channel, computer, and
//! (optionally) the full parsed `System`/`EventData` object. On top of the
//! parser it adds the filtering + aggregation an analyst actually wants —
//! filter by event id, provider, channel, and an inclusive ISO-8601 time range,
//! cap the number of returned records, and an aggregate `summary` mode that
//! returns counts by event id / provider / level and the file's time span
//! instead of the records themselves.
//!
//! No wafer/wasm-bindgen deps — shared verbatim by the chat skill block. Time
//! bounds are parsed with `jiff` (the same crate `evtx` stamps each record's
//! timestamp with) so range comparisons are exact, not string-lexicographic.

use evtx::{EvtxParser, ParserSettings};
use jiff::Timestamp;
use serde::Serialize;
use serde_json::Value;

/// Windows event severity level → human name (ETW `Level` field).
fn level_name(level: u64) -> Option<&'static str> {
    match level {
        0 => Some("Information"), // LogAlways, rendered as Information by Event Viewer
        1 => Some("Critical"),
        2 => Some("Error"),
        3 => Some("Warning"),
        4 => Some("Information"),
        5 => Some("Verbose"),
        _ => None,
    }
}

/// Parsed request options. Empty collections / `None` bounds mean "no filter".
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Max records to return (0 = all matched). Ignored in summary mode.
    pub max_records: usize,
    /// Only records whose EventID is in this set (empty = all).
    pub event_ids: Vec<u64>,
    /// Only records whose provider name contains one of these (case-insensitive
    /// substring; empty = all).
    pub providers: Vec<String>,
    /// Only records on this channel (case-insensitive exact; None = all).
    pub channel: Option<String>,
    /// Inclusive lower time bound (None = open).
    pub after: Option<Timestamp>,
    /// Inclusive upper time bound (None = open).
    pub before: Option<Timestamp>,
    /// Include the full parsed record object under `data` (default true).
    pub include_data: bool,
    /// Return aggregate counts instead of the records themselves.
    pub summary: bool,
}

/// One `(key, count)` aggregate row, emitted most-frequent first.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Count {
    pub key: String,
    pub count: usize,
}

/// One event record in the flat output list.
#[derive(Debug, Clone, Serialize)]
pub struct RecordOut {
    pub event_record_id: u64,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Aggregate view over the matched records (summary mode).
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub earliest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
    pub by_event_id: Vec<Count>,
    pub by_provider: Vec<Count>,
    pub by_level: Vec<Count>,
}

/// The full parse result.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// Every record the parser yielded, before any filter.
    pub total_records: usize,
    /// Records passing the filters, before the `max_records` cap.
    pub matched_records: usize,
    /// Records actually included in `records` (after the cap).
    pub returned_records: usize,
    /// Records the parser could not decode (corrupt chunks), skipped.
    pub parse_errors: usize,
    /// True when `matched_records > returned_records` (hit the cap).
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<Summary>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub records: Vec<RecordOut>,
}

/// Parse an ISO-8601 / RFC-3339 instant for the `after`/`before` bounds.
/// Accepts a full timestamp (`2016-06-29T15:24:34Z`, offset or `Z`) or a bare
/// calendar date (`2016-06-29`, interpreted as `T00:00:00Z`).
pub fn parse_bound(s: &str) -> Result<Timestamp, String> {
    let s = s.trim();
    if let Ok(ts) = s.parse::<Timestamp>() {
        return Ok(ts);
    }
    // Bare date → start of that UTC day.
    if s.len() == 10 && s.as_bytes()[4] == b'-' && s.as_bytes()[7] == b'-' {
        if let Ok(ts) = format!("{s}T00:00:00Z").parse::<Timestamp>() {
            return Ok(ts);
        }
    }
    Err(format!(
        "invalid timestamp {s:?}: use an ISO-8601 instant like 2016-06-29T15:24:34Z or a date like 2016-06-29"
    ))
}

/// Read a `u64` out of a JSON value that may be a bare number or an object with
/// a `#text` field (EVTX renders `EventID` with qualifiers as an object).
fn as_u64(v: &Value) -> Option<u64> {
    if let Some(n) = v.as_u64() {
        return Some(n);
    }
    v.get("#text").and_then(|t| t.as_u64())
}

fn system<'a>(data: &'a Value) -> Option<&'a Value> {
    data.get("Event")?.get("System")
}

/// Parse the buffer and apply the options. Errors only when the bytes are not a
/// valid EVTX file (bad magic / header); individual corrupt records are counted
/// in `parse_errors` and skipped.
pub fn parse(bytes: &[u8], opts: &Options) -> Result<Report, String> {
    let settings = ParserSettings::new()
        .separate_json_attributes(true)
        .num_threads(1);
    let mut parser = EvtxParser::from_buffer(bytes.to_vec())
        .map_err(|e| format!("not a valid .evtx file: {e}"))?
        .with_configuration(settings);

    let providers_lc: Vec<String> = opts.providers.iter().map(|p| p.to_lowercase()).collect();
    let channel_lc = opts.channel.as_ref().map(|c| c.to_lowercase());

    let mut total = 0usize;
    let mut matched = 0usize;
    let mut parse_errors = 0usize;
    let mut records: Vec<RecordOut> = Vec::new();

    // Aggregates (summary mode). Insertion-ordered maps kept as small vecs.
    let mut agg_event: Vec<Count> = Vec::new();
    let mut agg_provider: Vec<Count> = Vec::new();
    let mut agg_level: Vec<Count> = Vec::new();
    let mut earliest: Option<Timestamp> = None;
    let mut latest: Option<Timestamp> = None;

    for rec in parser.records_json_value() {
        let rec = match rec {
            Ok(r) => r,
            Err(_) => {
                parse_errors += 1;
                continue;
            }
        };
        total += 1;

        let sys = system(&rec.data);
        let event_id = sys.and_then(|s| s.get("EventID")).and_then(as_u64);
        let provider = sys
            .and_then(|s| s.get("Provider_attributes"))
            .and_then(|p| p.get("Name"))
            .and_then(|n| n.as_str())
            .map(|s| s.to_string());
        let level = sys.and_then(|s| s.get("Level")).and_then(as_u64);
        let channel = sys
            .and_then(|s| s.get("Channel"))
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());
        let computer = sys
            .and_then(|s| s.get("Computer"))
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());

        // --- filters ---
        if !opts.event_ids.is_empty() && !event_id.map_or(false, |id| opts.event_ids.contains(&id))
        {
            continue;
        }
        if !providers_lc.is_empty() {
            let p_lc = provider.as_deref().unwrap_or("").to_lowercase();
            if !providers_lc.iter().any(|needle| p_lc.contains(needle)) {
                continue;
            }
        }
        if let Some(ref want) = channel_lc {
            if channel.as_deref().unwrap_or("").to_lowercase() != *want {
                continue;
            }
        }
        if opts.after.is_some() || opts.before.is_some() {
            if let Some(lo) = opts.after {
                if rec.timestamp < lo {
                    continue;
                }
            }
            if let Some(hi) = opts.before {
                if rec.timestamp > hi {
                    continue;
                }
            }
        }

        // --- matched ---
        matched += 1;

        // time span over matched records
        earliest = Some(match earliest {
            Some(e) if e <= rec.timestamp => e,
            _ => rec.timestamp,
        });
        latest = Some(match latest {
            Some(l) if l >= rec.timestamp => l,
            _ => rec.timestamp,
        });

        if opts.summary {
            bump(&mut agg_event, &event_id.map_or("unknown".into(), |i| i.to_string()));
            bump(
                &mut agg_provider,
                provider.as_deref().unwrap_or("unknown"),
            );
            bump(
                &mut agg_level,
                &level
                    .map(|l| match level_name(l) {
                        Some(n) => format!("{l} ({n})"),
                        None => l.to_string(),
                    })
                    .unwrap_or_else(|| "unknown".into()),
            );
            continue;
        }

        // record output (respect the cap)
        if opts.max_records != 0 && records.len() >= opts.max_records {
            continue;
        }
        records.push(RecordOut {
            event_record_id: rec.event_record_id,
            timestamp: rec.timestamp.to_string(),
            event_id,
            provider,
            level,
            level_name: level.and_then(level_name).map(|s| s.to_string()),
            channel,
            computer,
            data: if opts.include_data {
                Some(rec.data)
            } else {
                None
            },
        });
    }

    let returned = records.len();
    let summary = if opts.summary {
        agg_event.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));
        agg_provider.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));
        agg_level.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));
        Some(Summary {
            earliest: earliest.map(|t| t.to_string()),
            latest: latest.map(|t| t.to_string()),
            by_event_id: agg_event,
            by_provider: agg_provider,
            by_level: agg_level,
        })
    } else {
        None
    };

    Ok(Report {
        total_records: total,
        matched_records: matched,
        returned_records: returned,
        parse_errors,
        truncated: matched > returned && !opts.summary,
        summary,
        records,
    })
}

/// Increment the count for `key` in an insertion-ordered aggregate vec.
fn bump(agg: &mut Vec<Count>, key: &str) {
    if let Some(c) = agg.iter_mut().find(|c| c.key == key) {
        c.count += 1;
    } else {
        agg.push(Count {
            key: key.to_string(),
            count: 1,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = include_bytes!("../tests/fixtures/security-sample.evtx");

    fn opts() -> Options {
        Options {
            include_data: true,
            ..Default::default()
        }
    }

    #[test]
    fn parses_all_records() {
        let r = parse(SAMPLE, &opts()).unwrap();
        assert_eq!(r.total_records, 7);
        assert_eq!(r.matched_records, 7);
        assert_eq!(r.returned_records, 7);
        assert_eq!(r.parse_errors, 0);
        assert!(!r.truncated);
        let first = &r.records[0];
        assert_eq!(first.event_record_id, 1);
        assert_eq!(first.event_id, Some(5152));
        assert_eq!(
            first.provider.as_deref(),
            Some("Microsoft-Windows-Security-Auditing")
        );
        assert_eq!(first.channel.as_deref(), Some("Security"));
        assert_eq!(first.computer.as_deref(), Some("temporal"));
        assert!(first.timestamp.starts_with("2016-06-29T15:24:34"));
        assert!(first.data.is_some());
    }

    #[test]
    fn filters_by_event_id() {
        let mut o = opts();
        o.event_ids = vec![5152];
        let r = parse(SAMPLE, &o).unwrap();
        assert_eq!(r.total_records, 7);
        assert!(r.matched_records >= 1);
        assert!(r.records.iter().all(|rec| rec.event_id == Some(5152)));
    }

    #[test]
    fn filters_by_provider_substring_ci() {
        let mut o = opts();
        o.providers = vec!["security-auditing".into()];
        let r = parse(SAMPLE, &o).unwrap();
        assert_eq!(r.matched_records, 7);
    }

    #[test]
    fn max_records_caps_and_marks_truncated() {
        let mut o = opts();
        o.max_records = 2;
        let r = parse(SAMPLE, &o).unwrap();
        assert_eq!(r.returned_records, 2);
        assert_eq!(r.matched_records, 7);
        assert!(r.truncated);
    }

    #[test]
    fn include_data_false_omits_body() {
        let mut o = opts();
        o.include_data = false;
        let r = parse(SAMPLE, &o).unwrap();
        assert!(r.records.iter().all(|rec| rec.data.is_none()));
    }

    #[test]
    fn time_range_filter() {
        // The fixture's records are all on 2016-06-29; an after-bound in the
        // future yields zero, an open range yields all.
        let mut o = opts();
        o.after = Some(parse_bound("2030-01-01T00:00:00Z").unwrap());
        let r = parse(SAMPLE, &o).unwrap();
        assert_eq!(r.matched_records, 0);

        let mut o2 = opts();
        o2.after = Some(parse_bound("2016-06-29").unwrap());
        o2.before = Some(parse_bound("2016-06-30").unwrap());
        let r2 = parse(SAMPLE, &o2).unwrap();
        assert_eq!(r2.matched_records, 7);
    }

    #[test]
    fn summary_mode_aggregates() {
        let mut o = opts();
        o.summary = true;
        let r = parse(SAMPLE, &o).unwrap();
        assert!(r.records.is_empty());
        let s = r.summary.unwrap();
        assert!(!s.by_event_id.is_empty());
        // counts sum to matched
        let sum: usize = s.by_event_id.iter().map(|c| c.count).sum();
        assert_eq!(sum, 7);
        assert!(s.earliest.is_some() && s.latest.is_some());
    }

    #[test]
    fn rejects_non_evtx_bytes() {
        let err = parse(b"not an event log at all", &opts()).unwrap_err();
        assert!(err.contains("not a valid .evtx file"));
    }

    #[test]
    fn parse_bound_accepts_forms_and_rejects_junk() {
        assert!(parse_bound("2016-06-29T15:24:34Z").is_ok());
        assert!(parse_bound("2016-06-29").is_ok());
        assert!(parse_bound("nonsense").is_err());
    }
}
