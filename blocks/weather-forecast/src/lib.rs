//! gizza-ai/weather-forecast — current conditions plus a multi-day (and
//! optionally hour-by-hour) forecast for any place, from the key-less
//! Open-Meteo APIs.
//!
//! Network block (same family as `web-fetch` / `http-request` /
//! `graphql-introspect`): it makes at most two `wafer-run/network` GETs — the
//! geocoding lookup that turns a place NAME into coordinates (skipped entirely
//! when the caller passes `lat,lon`), and the forecast itself. No page — chat
//! and CLI are the surfaces (see `.claude/skills/new-tool/SKILL.md` step 3,
//! "network — treat as a chat-only block"), because the shared page runtime
//! calls the browser export synchronously and cannot await a fetch.
//!
//! Everything except those two host requests is pure: location parsing, unit
//! groups, URL building, candidate selection, the WMO weather-code table, the
//! compass rose, response shaping and the summary line are plain functions
//! compiled (and unit-tested) on the host, exactly like `graphql-introspect`'s
//! renderers.
//!
//! No API key is ever sent — Open-Meteo's forecast and geocoding endpoints are
//! free and key-less. The only thing transmitted is the place name (or the
//! coordinates) and the caller's unit/day choices.

// The #[wafer_block] macro emits wasm-only registration; the host calls and the
// `Args` type are only used inside that impl. The pure helpers below are
// compiled (and unit-tested) on the host.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use std::collections::HashMap;

use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wafer_sdk::*;

/// Open-Meteo geocoding search (key-less). Turns a place name into coordinates.
const GEOCODE_BASE: &str = "https://geocoding-api.open-meteo.com/v1/search";
/// Open-Meteo forecast endpoint (key-less).
const FORECAST_BASE: &str = "https://api.open-meteo.com/v1/forecast";
/// Attribution echoed on every response.
const SOURCE: &str = "Open-Meteo (open-meteo.com), free key-less API";

/// How many geocoding candidates to fetch before filtering locally. Ambiguous
/// names (Springfield, Berlin, Cambridge) need a few rows to disambiguate from;
/// the API orders them by population, so the first match is the big one.
const GEOCODE_CANDIDATES: u32 = 10;

/// Unit groups, matching what forecast tools conventionally offer.
const ALLOWED_UNITS: [&str; 3] = ["metric", "us", "uk"];

/// Current-conditions variables requested from Open-Meteo.
const CURRENT_VARS: &str = "temperature_2m,relative_humidity_2m,apparent_temperature,is_day,\
                            precipitation,weather_code,cloud_cover,pressure_msl,surface_pressure,\
                            wind_speed_10m,wind_direction_10m,wind_gusts_10m";
/// Daily variables requested from Open-Meteo.
const DAILY_VARS: &str = "weather_code,temperature_2m_max,temperature_2m_min,\
                          apparent_temperature_max,apparent_temperature_min,sunrise,sunset,\
                          uv_index_max,precipitation_sum,precipitation_probability_max,\
                          wind_speed_10m_max,wind_gusts_10m_max,wind_direction_10m_dominant";
/// Hourly variables requested from Open-Meteo (only when `hours` > 0).
const HOURLY_VARS: &str = "temperature_2m,relative_humidity_2m,apparent_temperature,\
                           precipitation_probability,precipitation,weather_code,\
                           wind_speed_10m,wind_direction_10m";

/// Hard bounds on the caller-facing counts. Open-Meteo itself allows 16
/// forecast days; 48 hours is two days of hourly detail, which is as much as a
/// chat/CLI response can usefully carry.
const MAX_DAYS: i64 = 16;
const MAX_HOURS: i64 = 48;
const DEFAULT_DAYS: i64 = 7;

#[derive(Deserialize)]
struct Args {
    location: String,
    #[serde(default)]
    country: Option<String>,
    #[serde(default)]
    units: Option<String>,
    #[serde(default)]
    days: Option<i64>,
    #[serde(default)]
    hours: Option<i64>,
    #[serde(default)]
    timezone: Option<String>,
}

// ---------------------------------------------------------------------------
// Output shape
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct ResolvedLocation {
    /// Human-readable place label, e.g. `"Berlin, State of Berlin, Germany"`
    /// (or `"52.52, 13.41"` when coordinates were supplied directly).
    label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    /// First-level administrative area (state/region/province), when geocoded.
    #[serde(skip_serializing_if = "Option::is_none")]
    admin1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    country_code: Option<String>,
    latitude: f64,
    longitude: f64,
    /// Elevation of the forecast grid cell, in metres.
    #[serde(skip_serializing_if = "Option::is_none")]
    elevation: Option<f64>,
    /// IANA timezone the timestamps below are expressed in.
    timezone: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    timezone_abbreviation: Option<String>,
    utc_offset_seconds: i64,
}

#[derive(Debug, Serialize)]
struct Units {
    temperature: String,
    wind_speed: String,
    precipitation: String,
}

#[derive(Debug, Serialize)]
struct Current {
    /// Local time of the observation, `YYYY-MM-DDTHH:MM`.
    time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_day: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    weather_code: Option<i64>,
    /// Plain-language decode of `weather_code` (WMO table).
    conditions: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    apparent_temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    humidity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    precipitation: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cloud_cover: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pressure_msl: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    surface_pressure: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wind_speed: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wind_direction: Option<f64>,
    /// 16-point compass bearing derived from `wind_direction`.
    #[serde(skip_serializing_if = "Option::is_none")]
    wind_direction_cardinal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wind_gusts: Option<f64>,
}

#[derive(Debug, Serialize)]
struct DailyEntry {
    date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    weather_code: Option<i64>,
    conditions: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature_max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature_min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    apparent_temperature_max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    apparent_temperature_min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sunrise: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sunset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    uv_index_max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    precipitation_sum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    precipitation_probability_max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wind_speed_max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wind_gusts_max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wind_direction_dominant: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wind_direction_cardinal: Option<String>,
}

#[derive(Debug, Serialize)]
struct HourlyEntry {
    time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    weather_code: Option<i64>,
    conditions: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    apparent_temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    humidity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    precipitation: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    precipitation_probability: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wind_speed: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wind_direction: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wind_direction_cardinal: Option<String>,
}

#[derive(Debug, Serialize)]
struct ToolResp {
    location: ResolvedLocation,
    /// The unit group that was applied (`metric` / `us` / `uk`).
    units_group: String,
    /// The actual unit symbols the numbers below are in.
    units: Units,
    current: Current,
    daily: Vec<DailyEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hourly: Option<Vec<HourlyEntry>>,
    /// One-line, print-ready recap of `current` plus today's range.
    summary: String,
    /// Data attribution.
    source: &'static str,
}

// ---------------------------------------------------------------------------
// Descriptor (single source for the chat schema AND the CLI)
// ---------------------------------------------------------------------------

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("location")
                .required()
                .describe(
                    "Place to forecast: a city/town name (\"Berlin\"), a name with a \
                     disambiguating qualifier after a comma (\"Berlin, DE\", \"Springfield, IL\", \
                     \"Cambridge, United Kingdom\"), or raw coordinates as \"lat,lon\" \
                     (\"52.52,13.41\") which skip the geocoding lookup entirely.",
                ),
        )
        .param(
            Param::string("country")
                .describe(
                    "Optional country filter for the name lookup, as an ISO-3166 alpha-2 code \
                     (\"DE\", \"US\") or a country name (\"Germany\"). Use it when the same place \
                     name exists in several countries. Ignored when `location` is coordinates.",
                ),
        )
        .param(
            Param::enumv("units", ALLOWED_UNITS)
                .default("metric")
                .describe(
                    "Unit group: 'metric' (default) = °C, km/h, mm; 'us' = °F, mph, inch; \
                     'uk' = °C, mph, mm.",
                ),
        )
        .param(
            Param::integer("days")
                .min(1.0)
                .max(MAX_DAYS as f64)
                .default(DEFAULT_DAYS)
                .describe(
                    "How many days of daily forecast to return, 1-16. Day 1 is today. \
                     Default: 7.",
                ),
        )
        .param(
            Param::integer("hours")
                .min(0.0)
                .max(MAX_HOURS as f64)
                .default(0)
                .describe(
                    "How many hours of hour-by-hour detail to return, 0-48, counted forward from \
                     the current hour. Default: 0 (the `hourly` array is omitted entirely).",
                ),
        )
        .param(
            Param::string("timezone")
                .default("auto")
                .describe(
                    "Timezone for every returned timestamp: 'auto' (default) uses the location's \
                     own zone, or pass an IANA name such as 'Europe/Berlin' or 'UTC'.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

// ---------------------------------------------------------------------------
// Pure helpers — location parsing
// ---------------------------------------------------------------------------

/// What the caller's `location` resolved to before any network call.
#[derive(Debug, PartialEq)]
enum Target {
    /// Explicit coordinates — no geocoding needed.
    Coords { lat: f64, lon: f64 },
    /// A place name, plus the optional qualifier that followed a comma.
    Name {
        name: String,
        qualifier: Option<String>,
    },
}

/// Parse the `location` argument.
///
/// `"52.52,13.41"` (two numbers) is coordinates; anything else is a name whose
/// text after the first comma is a disambiguating qualifier. Coordinates are
/// range-checked here so a typo fails with a readable message instead of an
/// opaque upstream error.
fn parse_location(raw: &str) -> Result<Target, SkillError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(SkillError::InvalidArgs(
            "weather-forecast: 'location' must not be empty — pass a place name like \"Berlin\", \
             a qualified name like \"Springfield, IL\", or coordinates like \"52.52,13.41\""
                .to_string(),
        ));
    }

    let parts: Vec<&str> = trimmed.split(',').map(str::trim).collect();
    if parts.len() == 2 {
        if let (Ok(lat), Ok(lon)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
            if !(-90.0..=90.0).contains(&lat) {
                return Err(SkillError::InvalidArgs(format!(
                    "weather-forecast: latitude must be between -90 and 90, got {lat}"
                )));
            }
            if !(-180.0..=180.0).contains(&lon) {
                return Err(SkillError::InvalidArgs(format!(
                    "weather-forecast: longitude must be between -180 and 180, got {lon}"
                )));
            }
            return Ok(Target::Coords { lat, lon });
        }
    }

    let name = parts[0];
    if name.is_empty() {
        return Err(SkillError::InvalidArgs(format!(
            "weather-forecast: 'location' {trimmed:?} has no place name before the comma — \
             expected \"<place>\" or \"<place>, <qualifier>\""
        )));
    }
    let qualifier = {
        let rest = parts[1..]
            .iter()
            .filter(|p| !p.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(", ");
        if rest.is_empty() {
            None
        } else {
            Some(rest)
        }
    };
    Ok(Target::Name {
        name: name.to_string(),
        qualifier,
    })
}

// ---------------------------------------------------------------------------
// Pure helpers — unit groups
// ---------------------------------------------------------------------------

/// The three Open-Meteo unit query values a group maps to.
#[derive(Debug, PartialEq)]
struct UnitSpec {
    temperature: &'static str,
    wind_speed: &'static str,
    precipitation: &'static str,
}

/// Fallback unit SYMBOLS, used only if the API omits its `*_units` block.
impl UnitSpec {
    fn temperature_symbol(&self) -> &'static str {
        match self.temperature {
            "fahrenheit" => "°F",
            _ => "°C",
        }
    }
    fn wind_symbol(&self) -> &'static str {
        match self.wind_speed {
            "mph" => "mp/h",
            _ => "km/h",
        }
    }
    fn precipitation_symbol(&self) -> &'static str {
        match self.precipitation {
            "inch" => "inch",
            _ => "mm",
        }
    }
}

/// Validate + normalize the `units` group, returning the canonical name and the
/// per-field query values. `None` defaults to `metric`.
fn unit_spec(units: Option<&str>) -> Result<(String, UnitSpec), SkillError> {
    let raw = units.unwrap_or("metric").trim();
    let name = if raw.is_empty() {
        "metric".to_string()
    } else {
        raw.to_ascii_lowercase()
    };
    let spec = match name.as_str() {
        "metric" => UnitSpec {
            temperature: "celsius",
            wind_speed: "kmh",
            precipitation: "mm",
        },
        "us" => UnitSpec {
            temperature: "fahrenheit",
            wind_speed: "mph",
            precipitation: "inch",
        },
        "uk" => UnitSpec {
            temperature: "celsius",
            wind_speed: "mph",
            precipitation: "mm",
        },
        _ => {
            return Err(SkillError::InvalidArgs(format!(
                "weather-forecast: unsupported units {raw:?} (allowed: {})",
                ALLOWED_UNITS.join(", ")
            )))
        }
    };
    Ok((name, spec))
}

/// Validate `days` (1-16, default 7).
fn normalize_days(days: Option<i64>) -> Result<i64, SkillError> {
    let d = days.unwrap_or(DEFAULT_DAYS);
    if !(1..=MAX_DAYS).contains(&d) {
        return Err(SkillError::InvalidArgs(format!(
            "weather-forecast: 'days' must be between 1 and {MAX_DAYS}, got {d}"
        )));
    }
    Ok(d)
}

/// Validate `hours` (0-48, default 0 = no hourly block).
fn normalize_hours(hours: Option<i64>) -> Result<i64, SkillError> {
    let h = hours.unwrap_or(0);
    if !(0..=MAX_HOURS).contains(&h) {
        return Err(SkillError::InvalidArgs(format!(
            "weather-forecast: 'hours' must be between 0 and {MAX_HOURS}, got {h}"
        )));
    }
    Ok(h)
}

/// How many forecast DAYS to ask Open-Meteo for.
///
/// Hourly rows start at local midnight of day 1, but we hand back the next
/// `hours` hours counted from NOW — so at 23:00 local, 48 hours of detail needs
/// three days of hourly data. Ask for `ceil((23 + hours) / 24)` days at minimum
/// and slice the daily array back down to `days` locally.
fn forecast_days_needed(days: i64, hours: i64) -> i64 {
    if hours <= 0 {
        return days;
    }
    let needed = (hours + 46) / 24; // == ceil((23 + hours) / 24)
    days.max(needed).min(MAX_DAYS)
}

// ---------------------------------------------------------------------------
// Pure helpers — URL building
// ---------------------------------------------------------------------------

/// Percent-encode a query component per RFC 3986 (unreserved chars pass
/// through, everything else becomes `%XX`). Inlined rather than pulling in the
/// `url` crate — same approach as `http-request`, keeps the wasm small.
fn percent_encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => {
                out.push('%');
                out.push(hex_upper(b >> 4));
                out.push(hex_upper(b & 0x0f));
            }
        }
    }
    out
}

fn hex_upper(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'A' + (nibble - 10)) as char,
    }
}

/// Geocoding search URL for a place name.
fn build_geocode_url(name: &str) -> String {
    format!(
        "{GEOCODE_BASE}?name={}&count={GEOCODE_CANDIDATES}&language=en&format=json",
        percent_encode_component(name)
    )
}

/// Forecast URL. `hourly` is only requested when the caller asked for hours, so
/// the default call stays small.
fn build_forecast_url(
    lat: f64,
    lon: f64,
    spec: &UnitSpec,
    forecast_days: i64,
    want_hourly: bool,
    timezone: &str,
) -> String {
    let mut url = format!(
        "{FORECAST_BASE}?latitude={lat}&longitude={lon}\
         &current={CURRENT_VARS}&daily={DAILY_VARS}"
    );
    if want_hourly {
        url.push_str("&hourly=");
        url.push_str(HOURLY_VARS);
    }
    url.push_str(&format!(
        "&forecast_days={forecast_days}&timezone={}&temperature_unit={}&wind_speed_unit={}\
         &precipitation_unit={}",
        percent_encode_component(timezone),
        spec.temperature,
        spec.wind_speed,
        spec.precipitation
    ));
    url
}

/// Normalize the `timezone` argument (`None`/empty → `auto`).
fn normalize_timezone(tz: Option<&str>) -> String {
    let raw = tz.unwrap_or("auto").trim();
    if raw.is_empty() {
        "auto".to_string()
    } else {
        raw.to_string()
    }
}

// ---------------------------------------------------------------------------
// Pure helpers — WMO weather codes and the compass rose
// ---------------------------------------------------------------------------

/// Decode a WMO weather-interpretation code into plain English.
///
/// This is the code set Open-Meteo documents and actually emits. Anything
/// outside it is labelled rather than silently blanked, so a new upstream code
/// is visible instead of looking like "no data".
fn wmo_text(code: i64) -> String {
    let text = match code {
        0 => "Clear sky",
        1 => "Mainly clear",
        2 => "Partly cloudy",
        3 => "Overcast",
        45 => "Fog",
        48 => "Depositing rime fog",
        51 => "Light drizzle",
        53 => "Moderate drizzle",
        55 => "Dense drizzle",
        56 => "Light freezing drizzle",
        57 => "Dense freezing drizzle",
        61 => "Slight rain",
        63 => "Moderate rain",
        65 => "Heavy rain",
        66 => "Light freezing rain",
        67 => "Heavy freezing rain",
        71 => "Slight snowfall",
        73 => "Moderate snowfall",
        75 => "Heavy snowfall",
        77 => "Snow grains",
        80 => "Slight rain showers",
        81 => "Moderate rain showers",
        82 => "Violent rain showers",
        85 => "Slight snow showers",
        86 => "Heavy snow showers",
        95 => "Thunderstorm",
        96 => "Thunderstorm with slight hail",
        99 => "Thunderstorm with heavy hail",
        _ => return format!("Unknown weather code {code}"),
    };
    text.to_string()
}

/// Decode an optional code, for the places where the API may omit it.
fn conditions_for(code: Option<i64>) -> String {
    match code {
        Some(c) => wmo_text(c),
        None => "Unknown".to_string(),
    }
}

/// 16-point compass bearing for a wind direction in degrees. Degrees are
/// normalized first, so 370° and -350° both read `N`.
fn cardinal(degrees: f64) -> String {
    const POINTS: [&str; 16] = [
        "N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE", "S", "SSW", "SW", "WSW", "W", "WNW",
        "NW", "NNW",
    ];
    if !degrees.is_finite() {
        return "N".to_string();
    }
    let normalized = degrees.rem_euclid(360.0);
    let idx = ((normalized / 22.5).round() as usize) % 16;
    POINTS[idx].to_string()
}

fn cardinal_for(degrees: Option<f64>) -> Option<String> {
    degrees.map(cardinal)
}

// ---------------------------------------------------------------------------
// Pure helpers — geocoding candidate selection
// ---------------------------------------------------------------------------

/// A geocoded place, or the coordinates the caller supplied directly.
#[derive(Debug, Clone, PartialEq)]
struct Place {
    name: Option<String>,
    admin1: Option<String>,
    country: Option<String>,
    country_code: Option<String>,
    latitude: f64,
    longitude: f64,
}

impl Place {
    /// `"Berlin, State of Berlin, Germany"` — or the raw coordinates when the
    /// place was never geocoded.
    fn label(&self) -> String {
        let parts: Vec<&str> = [
            self.name.as_deref(),
            self.admin1.as_deref(),
            self.country.as_deref(),
        ]
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .collect();
        if parts.is_empty() {
            format!("{}, {}", self.latitude, self.longitude)
        } else {
            parts.join(", ")
        }
    }
}

/// US state/territory postal abbreviations, so `"Springfield, IL"` matches the
/// geocoder's `admin1 = "Illinois"`. The geocoding API returns full admin names
/// only, and a two-letter qualifier is how people actually write US places.
const US_STATES: [(&str, &str); 52] = [
    ("AL", "Alabama"),
    ("AK", "Alaska"),
    ("AZ", "Arizona"),
    ("AR", "Arkansas"),
    ("CA", "California"),
    ("CO", "Colorado"),
    ("CT", "Connecticut"),
    ("DE", "Delaware"),
    ("DC", "District of Columbia"),
    ("FL", "Florida"),
    ("GA", "Georgia"),
    ("HI", "Hawaii"),
    ("ID", "Idaho"),
    ("IL", "Illinois"),
    ("IN", "Indiana"),
    ("IA", "Iowa"),
    ("KS", "Kansas"),
    ("KY", "Kentucky"),
    ("LA", "Louisiana"),
    ("ME", "Maine"),
    ("MD", "Maryland"),
    ("MA", "Massachusetts"),
    ("MI", "Michigan"),
    ("MN", "Minnesota"),
    ("MS", "Mississippi"),
    ("MO", "Missouri"),
    ("MT", "Montana"),
    ("NE", "Nebraska"),
    ("NV", "Nevada"),
    ("NH", "New Hampshire"),
    ("NJ", "New Jersey"),
    ("NM", "New Mexico"),
    ("NY", "New York"),
    ("NC", "North Carolina"),
    ("ND", "North Dakota"),
    ("OH", "Ohio"),
    ("OK", "Oklahoma"),
    ("OR", "Oregon"),
    ("PA", "Pennsylvania"),
    ("PR", "Puerto Rico"),
    ("RI", "Rhode Island"),
    ("SC", "South Carolina"),
    ("SD", "South Dakota"),
    ("TN", "Tennessee"),
    ("TX", "Texas"),
    ("UT", "Utah"),
    ("VT", "Vermont"),
    ("VA", "Virginia"),
    ("WA", "Washington"),
    ("WV", "West Virginia"),
    ("WI", "Wisconsin"),
    ("WY", "Wyoming"),
];

/// Expand a two-letter US state abbreviation, if that's what `q` is.
fn expand_us_state(q: &str) -> Option<&'static str> {
    if q.len() != 2 {
        return None;
    }
    US_STATES
        .iter()
        .find(|(code, _)| code.eq_ignore_ascii_case(q))
        .map(|(_, name)| *name)
}

fn field_matches(field: Option<&str>, q: &str) -> bool {
    match field {
        Some(v) => v.eq_ignore_ascii_case(q) || v.to_lowercase().contains(&q.to_lowercase()),
        None => false,
    }
}

/// Does one geocoding candidate satisfy the caller's qualifier?
///
/// A qualifier matches on the country code, the country name, the first- or
/// second-level admin area, or the expansion of a US state abbreviation.
fn candidate_matches_qualifier(c: &Candidate, q: &str) -> bool {
    let q = q.trim();
    if q.is_empty() {
        return true;
    }
    if c.country_code
        .as_deref()
        .is_some_and(|cc| cc.eq_ignore_ascii_case(q))
    {
        return true;
    }
    if field_matches(c.country.as_deref(), q)
        || field_matches(c.admin1.as_deref(), q)
        || field_matches(c.admin2.as_deref(), q)
    {
        return true;
    }
    if let Some(full) = expand_us_state(q) {
        return field_matches(c.admin1.as_deref(), full);
    }
    false
}

/// Does one candidate satisfy the explicit `country` filter?
fn candidate_matches_country(c: &Candidate, country: &str) -> bool {
    let country = country.trim();
    if country.is_empty() {
        return true;
    }
    c.country_code
        .as_deref()
        .is_some_and(|cc| cc.eq_ignore_ascii_case(country))
        || field_matches(c.country.as_deref(), country)
}

/// One row of the geocoding API's `results` array.
#[derive(Debug, Clone, Deserialize)]
struct Candidate {
    name: Option<String>,
    latitude: f64,
    longitude: f64,
    #[serde(default)]
    country: Option<String>,
    #[serde(default)]
    country_code: Option<String>,
    #[serde(default)]
    admin1: Option<String>,
    #[serde(default)]
    admin2: Option<String>,
}

impl Candidate {
    fn describe(&self) -> String {
        let mut s = self.name.clone().unwrap_or_else(|| "?".to_string());
        if let Some(a) = self.admin1.as_deref().filter(|a| !a.is_empty()) {
            s.push_str(", ");
            s.push_str(a);
        }
        if let Some(c) = self.country.as_deref().filter(|c| !c.is_empty()) {
            s.push_str(", ");
            s.push_str(c);
        }
        if let Some(cc) = self.country_code.as_deref().filter(|c| !c.is_empty()) {
            s.push_str(&format!(" ({cc})"));
        }
        s
    }

    fn into_place(self) -> Place {
        Place {
            name: self.name,
            admin1: self.admin1,
            country: self.country,
            country_code: self.country_code,
            latitude: self.latitude,
            longitude: self.longitude,
        }
    }
}

/// Pick the best geocoding candidate for the query.
///
/// Results arrive population-ordered, so the first row that satisfies both the
/// qualifier and the `country` filter is the answer. When nothing matches, the
/// error lists what WAS found — the user can then pick a qualifier that works
/// instead of guessing.
fn select_place(
    body: &[u8],
    name_query: &str,
    qualifier: Option<&str>,
    country: Option<&str>,
) -> Result<Place, SkillError> {
    let parsed: Value = serde_json::from_slice(body).map_err(|e| {
        SkillError::Serialize(format!("parse geocoding response for {name_query:?}: {e}"))
    })?;
    let rows = parsed
        .get("results")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();

    let candidates: Vec<Candidate> = rows
        .into_iter()
        .filter_map(|v| serde_json::from_value::<Candidate>(v).ok())
        .collect();

    if candidates.is_empty() {
        return Err(SkillError::InvalidArgs(format!(
            "weather-forecast: no place named {name_query:?} was found — check the spelling, or \
             pass coordinates as \"lat,lon\" (e.g. \"52.52,13.41\")"
        )));
    }

    let matched = candidates.iter().find(|c| {
        qualifier.is_none_or(|q| candidate_matches_qualifier(c, q))
            && country.is_none_or(|k| candidate_matches_country(c, k))
    });

    match matched {
        Some(c) => Ok(c.clone().into_place()),
        None => {
            let filters = match (qualifier, country) {
                (Some(q), Some(k)) => format!("qualifier {q:?} and country {k:?}"),
                (Some(q), None) => format!("qualifier {q:?}"),
                (None, Some(k)) => format!("country {k:?}"),
                (None, None) => "the given filters".to_string(),
            };
            let listed = candidates
                .iter()
                .take(5)
                .map(Candidate::describe)
                .collect::<Vec<_>>()
                .join("; ");
            Err(SkillError::InvalidArgs(format!(
                "weather-forecast: no place named {name_query:?} matched {filters} — candidates \
                 found: {listed}"
            )))
        }
    }
}

// ---------------------------------------------------------------------------
// Pure helpers — response shaping
// ---------------------------------------------------------------------------

fn num_at(v: &Value, key: &str, i: usize) -> Option<f64> {
    v.get(key)?.as_array()?.get(i)?.as_f64()
}

fn int_at(v: &Value, key: &str, i: usize) -> Option<i64> {
    v.get(key)?.as_array()?.get(i)?.as_i64()
}

fn str_at(v: &Value, key: &str, i: usize) -> Option<String> {
    Some(v.get(key)?.as_array()?.get(i)?.as_str()?.to_string())
}

fn unit_of(units: Option<&Value>, key: &str, fallback: &str) -> String {
    units
        .and_then(|u| u.get(key))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

/// Round to one decimal and drop a trailing `.0`, so a summary reads
/// `18.4–26 °C` rather than `18.4–26.0 °C`.
fn fmt_num(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0;
    if r == r.trunc() {
        format!("{}", r.trunc() as i64)
    } else {
        format!("{r}")
    }
}

/// Index of the first hourly row at or after `current_time`.
///
/// Hourly rows start at local midnight; the caller asked for "the next N
/// hours", so slice forward from the current hour. Both timestamps are
/// same-format local ISO-8601, so the `YYYY-MM-DDTHH` prefix compares
/// lexicographically.
fn hourly_start_index(times: &[String], current_time: &str) -> usize {
    let cutoff = current_time.get(..13).unwrap_or(current_time);
    times
        .iter()
        .position(|t| t.get(..13).unwrap_or(t.as_str()) >= cutoff)
        .unwrap_or(0)
}

/// One-line recap: right now, plus today's range. This is what a terminal or a
/// chat reply prints when it doesn't want to render the whole structure.
fn build_summary(label: &str, current: &Current, today: Option<&DailyEntry>, units: &Units) -> String {
    let mut s = format!("{label} — now");
    match current.temperature {
        Some(t) => s.push_str(&format!(" {} {}", fmt_num(t), units.temperature)),
        None => s.push_str(" (temperature unavailable)"),
    }
    s.push_str(&format!(", {}", current.conditions.to_lowercase()));
    if let Some(a) = current.apparent_temperature {
        s.push_str(&format!(
            ", feels like {} {}",
            fmt_num(a),
            units.temperature
        ));
    }
    if let Some(w) = current.wind_speed {
        s.push_str(&format!(", wind {} {}", fmt_num(w), units.wind_speed));
        if let Some(c) = current.wind_direction_cardinal.as_deref() {
            s.push_str(&format!(" from {c}"));
        }
    }
    s.push('.');
    if let Some(day) = today {
        if let (Some(lo), Some(hi)) = (day.temperature_min, day.temperature_max) {
            s.push_str(&format!(
                " Today {}–{} {}, {}.",
                fmt_num(lo),
                fmt_num(hi),
                units.temperature,
                day.conditions.to_lowercase()
            ));
        }
    }
    s
}

/// Turn the forecast response into the tool's output envelope.
///
/// `place` carries whatever the geocoder resolved (nothing, for raw
/// coordinates); the timezone, elevation and unit symbols always come from the
/// forecast response itself, so a coordinate lookup is just as complete.
fn build_response(
    place: &Place,
    body: &[u8],
    units_group: &str,
    spec: &UnitSpec,
    days: i64,
    hours: i64,
) -> Result<ToolResp, SkillError> {
    let root: Value = serde_json::from_slice(body)
        .map_err(|e| SkillError::Serialize(format!("parse forecast response: {e}")))?;

    // Open-Meteo signals argument problems with `{"error": true, "reason": …}`.
    if let Some(reason) = root.get("reason").and_then(|r| r.as_str()) {
        return Err(SkillError::InvalidArgs(format!(
            "weather-forecast: the forecast API rejected the request: {reason}"
        )));
    }

    let current_units = root.get("current_units");
    let units = Units {
        temperature: unit_of(current_units, "temperature_2m", spec.temperature_symbol()),
        wind_speed: unit_of(current_units, "wind_speed_10m", spec.wind_symbol()),
        precipitation: unit_of(current_units, "precipitation", spec.precipitation_symbol()),
    };

    let cur = root.get("current").ok_or_else(|| {
        SkillError::Serialize("forecast response has no 'current' block".to_string())
    })?;
    let wind_direction = cur.get("wind_direction_10m").and_then(|v| v.as_f64());
    let current = Current {
        time: cur
            .get("time")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        is_day: cur.get("is_day").and_then(|v| v.as_i64()).map(|d| d == 1),
        weather_code: cur.get("weather_code").and_then(|v| v.as_i64()),
        conditions: conditions_for(cur.get("weather_code").and_then(|v| v.as_i64())),
        temperature: cur.get("temperature_2m").and_then(|v| v.as_f64()),
        apparent_temperature: cur.get("apparent_temperature").and_then(|v| v.as_f64()),
        humidity: cur.get("relative_humidity_2m").and_then(|v| v.as_f64()),
        precipitation: cur.get("precipitation").and_then(|v| v.as_f64()),
        cloud_cover: cur.get("cloud_cover").and_then(|v| v.as_f64()),
        pressure_msl: cur.get("pressure_msl").and_then(|v| v.as_f64()),
        surface_pressure: cur.get("surface_pressure").and_then(|v| v.as_f64()),
        wind_speed: cur.get("wind_speed_10m").and_then(|v| v.as_f64()),
        wind_direction,
        wind_direction_cardinal: cardinal_for(wind_direction),
        wind_gusts: cur.get("wind_gusts_10m").and_then(|v| v.as_f64()),
    };

    let empty = Value::Null;
    let daily_src = root.get("daily").unwrap_or(&empty);
    let day_count = daily_src
        .get("time")
        .and_then(|t| t.as_array())
        .map(|a| a.len())
        .unwrap_or(0)
        .min(days.max(0) as usize);
    let mut daily = Vec::with_capacity(day_count);
    for i in 0..day_count {
        let dir = num_at(daily_src, "wind_direction_10m_dominant", i);
        daily.push(DailyEntry {
            date: str_at(daily_src, "time", i).unwrap_or_default(),
            weather_code: int_at(daily_src, "weather_code", i),
            conditions: conditions_for(int_at(daily_src, "weather_code", i)),
            temperature_max: num_at(daily_src, "temperature_2m_max", i),
            temperature_min: num_at(daily_src, "temperature_2m_min", i),
            apparent_temperature_max: num_at(daily_src, "apparent_temperature_max", i),
            apparent_temperature_min: num_at(daily_src, "apparent_temperature_min", i),
            sunrise: str_at(daily_src, "sunrise", i),
            sunset: str_at(daily_src, "sunset", i),
            uv_index_max: num_at(daily_src, "uv_index_max", i),
            precipitation_sum: num_at(daily_src, "precipitation_sum", i),
            precipitation_probability_max: num_at(daily_src, "precipitation_probability_max", i),
            wind_speed_max: num_at(daily_src, "wind_speed_10m_max", i),
            wind_gusts_max: num_at(daily_src, "wind_gusts_10m_max", i),
            wind_direction_dominant: dir,
            wind_direction_cardinal: cardinal_for(dir),
        });
    }

    let hourly = if hours > 0 {
        let hourly_src = root.get("hourly").unwrap_or(&empty);
        let times: Vec<String> = hourly_src
            .get("time")
            .and_then(|t| t.as_array())
            .map(|a| {
                a.iter()
                    .map(|v| v.as_str().unwrap_or_default().to_string())
                    .collect()
            })
            .unwrap_or_default();
        let start = hourly_start_index(&times, &current.time);
        let end = (start + hours as usize).min(times.len());
        let mut rows = Vec::with_capacity(end.saturating_sub(start));
        for i in start..end {
            let dir = num_at(hourly_src, "wind_direction_10m", i);
            rows.push(HourlyEntry {
                time: times[i].clone(),
                weather_code: int_at(hourly_src, "weather_code", i),
                conditions: conditions_for(int_at(hourly_src, "weather_code", i)),
                temperature: num_at(hourly_src, "temperature_2m", i),
                apparent_temperature: num_at(hourly_src, "apparent_temperature", i),
                humidity: num_at(hourly_src, "relative_humidity_2m", i),
                precipitation: num_at(hourly_src, "precipitation", i),
                precipitation_probability: num_at(hourly_src, "precipitation_probability", i),
                wind_speed: num_at(hourly_src, "wind_speed_10m", i),
                wind_direction: dir,
                wind_direction_cardinal: cardinal_for(dir),
            });
        }
        Some(rows)
    } else {
        None
    };

    let location = ResolvedLocation {
        label: place.label(),
        name: place.name.clone(),
        admin1: place.admin1.clone(),
        country: place.country.clone(),
        country_code: place.country_code.clone(),
        latitude: root
            .get("latitude")
            .and_then(|v| v.as_f64())
            .unwrap_or(place.latitude),
        longitude: root
            .get("longitude")
            .and_then(|v| v.as_f64())
            .unwrap_or(place.longitude),
        elevation: root.get("elevation").and_then(|v| v.as_f64()),
        timezone: root
            .get("timezone")
            .and_then(|v| v.as_str())
            .unwrap_or("UTC")
            .to_string(),
        timezone_abbreviation: root
            .get("timezone_abbreviation")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        utc_offset_seconds: root
            .get("utc_offset_seconds")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
    };

    let summary = build_summary(&location.label, &current, daily.first(), &units);

    Ok(ToolResp {
        location,
        units_group: units_group.to_string(),
        units,
        current,
        daily,
        hourly,
        summary,
        source: SOURCE,
    })
}

// ---------------------------------------------------------------------------
// Block registration + the two host requests
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
struct WeatherForecast;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/weather-forecast",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Get current conditions and a multi-day forecast for any place using the free, key-less Open-Meteo API.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Get the current weather and a multi-day forecast for any place on Earth from the free, key-less Open-Meteo API. Pass a city name (\"Berlin\"), a name with a qualifier (\"Springfield, IL\", \"Cambridge, United Kingdom\") or raw coordinates (\"52.52,13.41\"); optionally filter the name lookup by country. Choose a unit group (metric °C/km/h/mm, us °F/mph/inch, uk °C/mph/mm), 1-16 forecast days, and 0-48 hours of hour-by-hour detail counted from the current hour. Returns the resolved location, current conditions with plain-language WMO weather text and a 16-point compass wind direction, a daily forecast with highs/lows, sunrise/sunset, UV index, precipitation totals and probabilities, an optional hourly array, and a one-line summary. No API key is used or required.",
        parameters = schema_json()
    ),
)]
impl WeatherForecast {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // Returns the flat ToolResp JSON directly (no `{ "result": … }`
        // wrapper), same as web-fetch / http-request / graphql-introspect.
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

/// Fetch a URL with the shared network block and require a 2xx.
#[cfg(target_arch = "wasm32")]
fn get(url: &str) -> Result<Vec<u8>, SkillError> {
    let headers: HashMap<String, String> = HashMap::new();
    let resp = wafer_sdk::clients::network::do_request("GET", url, &headers, None)?;
    if resp.status_code < 200 || resp.status_code >= 300 {
        return Err(SkillError::HttpStatus {
            status: resp.status_code,
            url: url.to_string(),
        });
    }
    Ok(resp.body)
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("weather-forecast")?;

    let target = parse_location(&args.location)?;
    let (units_group, spec) = unit_spec(args.units.as_deref())?;
    let days = normalize_days(args.days)?;
    let hours = normalize_hours(args.hours)?;
    let timezone = normalize_timezone(args.timezone.as_deref());

    // Step 1 — resolve a place NAME to coordinates. Skipped for "lat,lon".
    let place = match target {
        Target::Coords { lat, lon } => Place {
            name: None,
            admin1: None,
            country: None,
            country_code: None,
            latitude: lat,
            longitude: lon,
        },
        Target::Name { name, qualifier } => {
            let geo = get(&build_geocode_url(&name))?;
            select_place(
                &geo,
                &name,
                qualifier.as_deref(),
                args.country.as_deref().filter(|c| !c.trim().is_empty()),
            )?
        }
    };

    // Step 2 — the forecast itself.
    let url = build_forecast_url(
        place.latitude,
        place.longitude,
        &spec,
        forecast_days_needed(days, hours),
        hours > 0,
        &timezone,
    );
    let forecast = get(&url)?;

    let tool = build_response(&place, &forecast, &units_group, &spec, days, hours)?;
    serde_json::to_vec(&tool)
        .map_err(|e| SkillError::Serialize(format!("serialize tool response: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trimmed but structurally real Open-Meteo forecast response (Berlin, 3
    /// days, hourly). Captured from the live API, so the parsing tests below
    /// exercise the actual field names and shapes.
    const FORECAST_FIXTURE: &str = include_str!("../tests/fixtures/forecast-berlin.json");
    /// A real geocoding response for "Berlin" — one German, two US rows.
    const GEOCODE_FIXTURE: &str = include_str!("../tests/fixtures/geocode-berlin.json");

    fn metric() -> UnitSpec {
        unit_spec(Some("metric")).unwrap().1
    }

    // --- chat-schema drift guard --------------------------------------------

    /// The descriptor is the single source for the chat schema AND the CLI, so
    /// pin its rendered JSON: any accidental param rename/removal fails here.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "location": { "type": "string", "description": "Place to forecast: a city/town name (\"Berlin\"), a name with a disambiguating qualifier after a comma (\"Berlin, DE\", \"Springfield, IL\", \"Cambridge, United Kingdom\"), or raw coordinates as \"lat,lon\" (\"52.52,13.41\") which skip the geocoding lookup entirely." },
                    "country": { "type": "string", "description": "Optional country filter for the name lookup, as an ISO-3166 alpha-2 code (\"DE\", \"US\") or a country name (\"Germany\"). Use it when the same place name exists in several countries. Ignored when `location` is coordinates." },
                    "units": { "type": "string", "enum": ["metric", "us", "uk"], "default": "metric", "description": "Unit group: 'metric' (default) = °C, km/h, mm; 'us' = °F, mph, inch; 'uk' = °C, mph, mm." },
                    "days": { "type": "integer", "minimum": 1, "maximum": 16, "default": 7, "description": "How many days of daily forecast to return, 1-16. Day 1 is today. Default: 7." },
                    "hours": { "type": "integer", "minimum": 0, "maximum": 48, "default": 0, "description": "How many hours of hour-by-hour detail to return, 0-48, counted forward from the current hour. Default: 0 (the `hourly` array is omitted entirely)." },
                    "timezone": { "type": "string", "default": "auto", "description": "Timezone for every returned timestamp: 'auto' (default) uses the location's own zone, or pass an IANA name such as 'Europe/Berlin' or 'UTC'." }
                },
                "required": ["location"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    // --- happy path: a real response becomes the output envelope ------------

    #[test]
    fn build_response_shapes_a_real_forecast() {
        let place = Place {
            name: Some("Berlin".into()),
            admin1: Some("State of Berlin".into()),
            country: Some("Germany".into()),
            country_code: Some("DE".into()),
            latitude: 52.52437,
            longitude: 13.41053,
        };
        let resp = build_response(
            &place,
            FORECAST_FIXTURE.as_bytes(),
            "metric",
            &metric(),
            2,
            3,
        )
        .expect("fixture parses");

        // Resolved location: geocoder fields plus the forecast's own zone/elevation.
        assert_eq!(resp.location.label, "Berlin, State of Berlin, Germany");
        assert_eq!(resp.location.country_code.as_deref(), Some("DE"));
        assert_eq!(resp.location.timezone, "Europe/Berlin");
        assert_eq!(resp.location.utc_offset_seconds, 7200);
        assert_eq!(resp.location.elevation, Some(38.0));

        // Units come from the API's own `current_units` block.
        assert_eq!(resp.units.temperature, "°C");
        assert_eq!(resp.units.wind_speed, "km/h");
        assert_eq!(resp.units.precipitation, "mm");
        assert_eq!(resp.units_group, "metric");

        // Current conditions, incl. the WMO decode and the compass bearing.
        assert_eq!(resp.current.time, "2026-08-29T21:30");
        assert_eq!(resp.current.temperature, Some(23.4));
        assert_eq!(resp.current.weather_code, Some(3));
        assert_eq!(resp.current.conditions, "Overcast");
        assert_eq!(resp.current.wind_direction, Some(207.0));
        assert_eq!(resp.current.wind_direction_cardinal.as_deref(), Some("SSW"));
        assert_eq!(resp.current.is_day, Some(false));

        // `days` slices the daily array even though 3 days were fetched.
        assert_eq!(resp.daily.len(), 2);
        assert_eq!(resp.daily[0].date, "2026-08-29");
        assert_eq!(resp.daily[0].temperature_max, Some(26.2));
        assert_eq!(resp.daily[0].sunrise.as_deref(), Some("2026-08-29T06:12"));
        assert_eq!(resp.daily[1].conditions, "Slight rain");
        assert_eq!(resp.daily[1].wind_direction_cardinal.as_deref(), Some("SW"));

        // Hourly starts at the CURRENT hour (21:00), not local midnight.
        let hourly = resp.hourly.as_ref().expect("hours=3 returns an hourly array");
        assert_eq!(hourly.len(), 3);
        assert_eq!(hourly[0].time, "2026-08-29T21:00");
        assert_eq!(hourly[0].temperature, Some(23.6));
        assert_eq!(hourly[0].conditions, "Overcast");
        assert_eq!(hourly[2].time, "2026-08-29T23:00");

        assert_eq!(
            resp.summary,
            "Berlin, State of Berlin, Germany — now 23.4 °C, overcast, feels like 22.7 °C, \
             wind 8 km/h from SSW. Today 18.4–26.2 °C, overcast."
        );
        assert!(resp.source.contains("Open-Meteo"));
    }

    #[test]
    fn hours_zero_omits_the_hourly_array() {
        let place = Place {
            name: Some("Berlin".into()),
            admin1: None,
            country: None,
            country_code: None,
            latitude: 52.52,
            longitude: 13.41,
        };
        let resp =
            build_response(&place, FORECAST_FIXTURE.as_bytes(), "metric", &metric(), 3, 0).unwrap();
        assert!(resp.hourly.is_none(), "hours=0 omits hourly entirely");
        assert_eq!(resp.daily.len(), 3);
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json.get("hourly").is_none(), "omitted, not null");
    }

    // --- error paths --------------------------------------------------------

    #[test]
    fn empty_location_is_rejected() {
        let err = parse_location("   ").unwrap_err();
        assert!(
            err.to_string().contains("'location' must not be empty"),
            "message says what was expected: {err}"
        );
    }

    #[test]
    fn out_of_range_coordinates_are_rejected() {
        let err = parse_location("95.0,13.41").unwrap_err();
        assert!(err.to_string().contains("latitude must be between -90 and 90"));
        let err = parse_location("52.52,200").unwrap_err();
        assert!(err
            .to_string()
            .contains("longitude must be between -180 and 180"));
    }

    #[test]
    fn unknown_unit_group_is_rejected() {
        let err = unit_spec(Some("kelvin")).unwrap_err();
        assert!(err.to_string().contains("unsupported units \"kelvin\""));
        assert!(err.to_string().contains("metric, us, uk"));
    }

    #[test]
    fn out_of_range_days_and_hours_are_rejected() {
        assert!(normalize_days(Some(0)).is_err());
        assert!(normalize_days(Some(17)).is_err());
        assert!(normalize_hours(Some(-1)).is_err());
        let err = normalize_hours(Some(49)).unwrap_err();
        assert!(err.to_string().contains("'hours' must be between 0 and 48"));
    }

    #[test]
    fn forecast_api_error_body_becomes_a_readable_error() {
        let place = Place {
            name: None,
            admin1: None,
            country: None,
            country_code: None,
            latitude: 0.0,
            longitude: 0.0,
        };
        let body = br#"{"error":true,"reason":"Parameter 'forecast_days' is out of allowed range"}"#;
        let err = build_response(&place, body, "metric", &metric(), 7, 0).unwrap_err();
        assert!(err.to_string().contains("the forecast API rejected"));
        assert!(err.to_string().contains("out of allowed range"));
    }

    #[test]
    fn geocoding_with_no_results_is_rejected() {
        let err = select_place(br#"{"generationtime_ms":0.2}"#, "Zzzqqq", None, None).unwrap_err();
        assert!(err.to_string().contains("no place named \"Zzzqqq\" was found"));
        assert!(err.to_string().contains("lat,lon"));
    }

    #[test]
    fn geocoding_with_no_matching_qualifier_lists_the_candidates() {
        let err = select_place(GEOCODE_FIXTURE.as_bytes(), "Berlin", Some("FR"), None).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("matched qualifier \"FR\""));
        assert!(msg.contains("Berlin, State of Berlin, Germany (DE)"));
    }

    // --- location parsing ---------------------------------------------------

    #[test]
    fn parses_plain_names_qualifiers_and_coordinates() {
        assert_eq!(
            parse_location("Berlin").unwrap(),
            Target::Name {
                name: "Berlin".into(),
                qualifier: None
            }
        );
        assert_eq!(
            parse_location(" Springfield , IL ").unwrap(),
            Target::Name {
                name: "Springfield".into(),
                qualifier: Some("IL".into())
            }
        );
        assert_eq!(
            parse_location("Cambridge, Cambridgeshire, United Kingdom").unwrap(),
            Target::Name {
                name: "Cambridge".into(),
                qualifier: Some("Cambridgeshire, United Kingdom".into())
            }
        );
        assert_eq!(
            parse_location("52.52,13.41").unwrap(),
            Target::Coords {
                lat: 52.52,
                lon: 13.41
            }
        );
        assert_eq!(
            parse_location("-33.87, 151.21").unwrap(),
            Target::Coords {
                lat: -33.87,
                lon: 151.21
            }
        );
    }

    // --- candidate selection ------------------------------------------------

    #[test]
    fn qualifier_picks_the_right_berlin() {
        let de = select_place(GEOCODE_FIXTURE.as_bytes(), "Berlin", Some("DE"), None).unwrap();
        assert_eq!(de.country_code.as_deref(), Some("DE"));
        assert_eq!(de.latitude, 52.52437);

        let nh =
            select_place(GEOCODE_FIXTURE.as_bytes(), "Berlin", Some("New Hampshire"), None).unwrap();
        assert_eq!(nh.admin1.as_deref(), Some("New Hampshire"));

        // Two-letter US state abbreviation expands to the geocoder's admin1.
        let nj = select_place(GEOCODE_FIXTURE.as_bytes(), "Berlin", Some("NJ"), None).unwrap();
        assert_eq!(nj.admin1.as_deref(), Some("New Jersey"));
    }

    #[test]
    fn country_filter_narrows_without_a_qualifier() {
        let us = select_place(GEOCODE_FIXTURE.as_bytes(), "Berlin", None, Some("US")).unwrap();
        assert_eq!(us.country_code.as_deref(), Some("US"));
        // Population order decides among the US rows.
        assert_eq!(us.admin1.as_deref(), Some("New Hampshire"));
    }

    #[test]
    fn no_filters_takes_the_most_populous_candidate() {
        let first = select_place(GEOCODE_FIXTURE.as_bytes(), "Berlin", None, None).unwrap();
        assert_eq!(first.country.as_deref(), Some("Germany"));
        assert_eq!(first.label(), "Berlin, State of Berlin, Germany");
    }

    // --- unit groups, URLs, day math ---------------------------------------

    #[test]
    fn unit_groups_map_to_open_meteo_query_values() {
        assert_eq!(unit_spec(None).unwrap().0, "metric");
        let (name, us) = unit_spec(Some("US")).unwrap();
        assert_eq!(name, "us");
        assert_eq!(us.temperature, "fahrenheit");
        assert_eq!(us.wind_speed, "mph");
        assert_eq!(us.precipitation, "inch");
        let (_, uk) = unit_spec(Some("uk")).unwrap();
        assert_eq!(uk.temperature, "celsius");
        assert_eq!(uk.wind_speed, "mph");
        assert_eq!(uk.precipitation, "mm");
    }

    #[test]
    fn forecast_url_carries_units_days_timezone_and_optional_hourly() {
        let url = build_forecast_url(52.52, 13.41, &metric(), 7, false, "auto");
        assert!(url.starts_with("https://api.open-meteo.com/v1/forecast?latitude=52.52&longitude=13.41"));
        assert!(url.contains("&forecast_days=7"));
        assert!(url.contains("&timezone=auto"));
        assert!(url.contains("&temperature_unit=celsius"));
        assert!(url.contains("&wind_speed_unit=kmh"));
        assert!(url.contains("&precipitation_unit=mm"));
        assert!(!url.contains("&hourly="), "hourly is opt-in");

        let (_, us) = unit_spec(Some("us")).unwrap();
        let url = build_forecast_url(40.71, -74.0, &us, 3, true, "America/New_York");
        assert!(url.contains("&hourly=temperature_2m"));
        assert!(url.contains("&temperature_unit=fahrenheit"));
        assert!(url.contains("&timezone=America%2FNew_York"));
    }

    #[test]
    fn geocode_url_percent_encodes_the_name() {
        let url = build_geocode_url("São Paulo");
        assert_eq!(
            url,
            "https://geocoding-api.open-meteo.com/v1/search?name=S%C3%A3o%20Paulo&count=10&language=en&format=json"
        );
    }

    #[test]
    fn hourly_detail_forces_enough_forecast_days() {
        assert_eq!(forecast_days_needed(7, 0), 7, "no hourly, no change");
        assert_eq!(forecast_days_needed(1, 1), 1);
        assert_eq!(forecast_days_needed(1, 24), 2);
        assert_eq!(forecast_days_needed(1, 48), 3, "48h from 23:00 spans 3 days");
        assert_eq!(forecast_days_needed(10, 48), 10, "days already covers it");
        assert_eq!(forecast_days_needed(16, 48), 16, "never exceeds the cap");
    }

    // --- WMO table, compass rose, formatting --------------------------------

    #[test]
    fn wmo_codes_decode_to_plain_language() {
        assert_eq!(wmo_text(0), "Clear sky");
        assert_eq!(wmo_text(3), "Overcast");
        assert_eq!(wmo_text(45), "Fog");
        assert_eq!(wmo_text(61), "Slight rain");
        assert_eq!(wmo_text(75), "Heavy snowfall");
        assert_eq!(wmo_text(95), "Thunderstorm");
        assert_eq!(wmo_text(99), "Thunderstorm with heavy hail");
        // Unknown codes are labelled, not blanked.
        assert_eq!(wmo_text(7), "Unknown weather code 7");
        assert_eq!(conditions_for(None), "Unknown");
    }

    #[test]
    fn compass_covers_all_sixteen_points_and_wraps() {
        assert_eq!(cardinal(0.0), "N");
        assert_eq!(cardinal(90.0), "E");
        assert_eq!(cardinal(180.0), "S");
        assert_eq!(cardinal(270.0), "W");
        assert_eq!(cardinal(22.5), "NNE");
        assert_eq!(cardinal(207.0), "SSW");
        assert_eq!(cardinal(239.0), "WSW");
        assert_eq!(cardinal(350.0), "N", "wraps past 348.75");
        assert_eq!(cardinal(370.0), "N", "normalizes over 360");
        assert_eq!(cardinal(-90.0), "W", "normalizes negatives");
    }

    #[test]
    fn numbers_drop_a_trailing_zero_decimal() {
        assert_eq!(fmt_num(23.4), "23.4");
        assert_eq!(fmt_num(26.0), "26");
        assert_eq!(fmt_num(-3.25), "-3.3", "halves round away from zero");
        assert_eq!(fmt_num(0.0), "0");
    }

    #[test]
    fn hourly_slice_starts_at_the_current_hour() {
        let times: Vec<String> = (0..24)
            .map(|h| format!("2026-08-29T{h:02}:00"))
            .collect();
        assert_eq!(hourly_start_index(&times, "2026-08-29T00:10"), 0);
        assert_eq!(hourly_start_index(&times, "2026-08-29T21:30"), 21);
        assert_eq!(hourly_start_index(&times, "2026-08-29T23:59"), 23);
        // A current time past every row falls back to the start rather than panicking.
        assert_eq!(hourly_start_index(&times, "2026-08-30T05:00"), 0);
    }

    #[test]
    fn timezone_defaults_to_auto() {
        assert_eq!(normalize_timezone(None), "auto");
        assert_eq!(normalize_timezone(Some("  ")), "auto");
        assert_eq!(normalize_timezone(Some(" Europe/Berlin ")), "Europe/Berlin");
    }
}
