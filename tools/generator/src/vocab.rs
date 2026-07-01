//! Named option vocabularies for `[[input]] options = "<name>"` — rendered as a
//! `<datalist>` so large standard value sets (timezones, …) get searchable
//! autocomplete instead of a raw text box. Declarative: a tool opts in from its
//! `page/meta.toml`; no per-tool JS.

/// IANA timezone names, matching the set the timezone tools accept. Kept short on
/// purpose (major zones per region) — a datalist is autocomplete, not validation;
/// the core still accepts any valid IANA name typed in full.
pub const TIMEZONES: &[&str] = &[
    "UTC", "GMT",
    "Africa/Cairo", "Africa/Johannesburg", "Africa/Lagos", "Africa/Nairobi",
    "America/Anchorage", "America/Argentina/Buenos_Aires", "America/Bogota",
    "America/Chicago", "America/Denver", "America/Halifax", "America/Los_Angeles",
    "America/Mexico_City", "America/New_York", "America/Phoenix", "America/Santiago",
    "America/Sao_Paulo", "America/St_Johns", "America/Toronto", "America/Vancouver",
    "Asia/Bangkok", "Asia/Dubai", "Asia/Hong_Kong", "Asia/Jakarta", "Asia/Jerusalem",
    "Asia/Kabul", "Asia/Kolkata", "Asia/Kathmandu", "Asia/Manila", "Asia/Riyadh",
    "Asia/Seoul", "Asia/Shanghai", "Asia/Singapore", "Asia/Taipei", "Asia/Tashkent",
    "Asia/Tehran", "Asia/Tokyo", "Atlantic/Azores",
    "Australia/Adelaide", "Australia/Brisbane", "Australia/Darwin", "Australia/Hobart",
    "Australia/Melbourne", "Australia/Perth", "Australia/Sydney",
    "Europe/Amsterdam", "Europe/Athens", "Europe/Belgrade", "Europe/Berlin",
    "Europe/Brussels", "Europe/Budapest", "Europe/Copenhagen", "Europe/Dublin",
    "Europe/Helsinki", "Europe/Istanbul", "Europe/Lisbon", "Europe/London",
    "Europe/Madrid", "Europe/Moscow", "Europe/Oslo", "Europe/Paris", "Europe/Prague",
    "Europe/Rome", "Europe/Stockholm", "Europe/Vienna", "Europe/Warsaw", "Europe/Zurich",
    "Pacific/Auckland", "Pacific/Chatham", "Pacific/Fiji", "Pacific/Honolulu",
];

/// Look up a named vocabulary. Unknown names return `None` and the field falls
/// back to a plain text input (the generator warns, it does not abort).
pub fn options(name: &str) -> Option<&'static [&'static str]> {
    match name {
        "timezones" => Some(TIMEZONES),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timezones_vocab_resolves() {
        let tz = options("timezones").expect("timezones vocab exists");
        assert!(tz.contains(&"UTC"));
        assert!(tz.contains(&"Europe/Amsterdam"));
    }

    #[test]
    fn unknown_vocab_is_none() {
        assert_eq!(options("nope"), None);
    }
}
