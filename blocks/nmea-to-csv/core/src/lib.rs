//! gizza-ai/nmea-to-csv core — turn a raw NMEA 0183 GPS/GNSS serial log into a
//! CSV track table, one row per positional fix. Sentences that belong to the
//! same cycle (share a UTC time-of-day) are merged into a single row: GGA
//! contributes altitude / fix quality / satellite count / HDOP, RMC contributes
//! date / speed / course, GLL is a position fallback, VTG fills speed + course,
//! ZDA supplies the date, and GSA backfills HDOP. Pure Rust, no wafer /
//! wasm-bindgen deps — shared by the chat block, the CLI, and the page.

/// Coordinate output format.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Coordinates {
    Decimal,
    Dms,
}

impl Coordinates {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "decimal" | "dd" => Ok(Coordinates::Decimal),
            "dms" => Ok(Coordinates::Dms),
            other => Err(format!(
                "invalid coordinates '{other}': expected one of decimal, dms"
            )),
        }
    }
}

/// Altitude output unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AltitudeUnit {
    Meters,
    Feet,
}

impl AltitudeUnit {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "m" | "meters" | "metres" => Ok(AltitudeUnit::Meters),
            "ft" | "feet" | "foot" => Ok(AltitudeUnit::Feet),
            other => Err(format!(
                "invalid altitude_unit '{other}': expected one of m, ft"
            )),
        }
    }
    fn suffix(self) -> &'static str {
        match self {
            AltitudeUnit::Meters => "m",
            AltitudeUnit::Feet => "ft",
        }
    }
    /// Convert a value in metres to this unit.
    fn from_meters(self, m: f64) -> f64 {
        match self {
            AltitudeUnit::Meters => m,
            AltitudeUnit::Feet => m * 3.280_839_895,
        }
    }
}

/// Speed output unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpeedUnit {
    Knots,
    Kmh,
    Mph,
}

impl SpeedUnit {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "knots" | "kn" | "kt" => Ok(SpeedUnit::Knots),
            "kmh" | "kph" | "km/h" | "kmph" => Ok(SpeedUnit::Kmh),
            "mph" => Ok(SpeedUnit::Mph),
            other => Err(format!(
                "invalid speed_unit '{other}': expected one of knots, kmh, mph"
            )),
        }
    }
    fn suffix(self) -> &'static str {
        match self {
            SpeedUnit::Knots => "knots",
            SpeedUnit::Kmh => "kmh",
            SpeedUnit::Mph => "mph",
        }
    }
    /// Convert a value in knots to this unit.
    fn from_knots(self, kn: f64) -> f64 {
        match self {
            SpeedUnit::Knots => kn,
            SpeedUnit::Kmh => kn * 1.852,
            SpeedUnit::Mph => kn * 1.150_779_448,
        }
    }
}

/// Output field separator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Delimiter {
    Comma,
    Semicolon,
    Tab,
    Pipe,
}

impl Delimiter {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "comma" | "," => Ok(Delimiter::Comma),
            "semicolon" | ";" => Ok(Delimiter::Semicolon),
            "tab" | "\t" => Ok(Delimiter::Tab),
            "pipe" | "|" => Ok(Delimiter::Pipe),
            other => Err(format!(
                "invalid delimiter '{other}': expected one of comma, semicolon, tab, pipe"
            )),
        }
    }
    fn ch(self) -> char {
        match self {
            Delimiter::Comma => ',',
            Delimiter::Semicolon => ';',
            Delimiter::Tab => '\t',
            Delimiter::Pipe => '|',
        }
    }
}

/// One merged positional fix (one output row).
#[derive(Debug, Clone, Default)]
struct Record {
    /// UTC time-of-day, integer part only (`hhmmss`), the cycle key.
    time: String,
    /// Full date as (year, month, day) once an RMC/ZDA has supplied it.
    date: Option<(i32, u32, u32)>,
    /// Signed decimal-degree coordinates.
    lat: Option<f64>,
    lon: Option<f64>,
    /// Altitude above mean sea level, metres (GGA field 9).
    altitude_m: Option<f64>,
    /// GGA fix-quality indicator (0..=8).
    fix: Option<u8>,
    /// Satellites used in the fix (GGA field 7).
    satellites: Option<u32>,
    /// Horizontal dilution of precision, kept as raw text (GGA field 8 / GSA).
    hdop: Option<String>,
    /// Speed over ground in knots (RMC field 7 / VTG field 5).
    speed_knots: Option<f64>,
    /// Course over ground, degrees true (RMC field 8 / VTG field 1).
    course: Option<f64>,
}

impl Record {
    fn has_position(&self) -> bool {
        self.lat.is_some() && self.lon.is_some()
    }
}

/// Human-readable label for a GGA fix-quality indicator.
fn fix_label(q: u8) -> &'static str {
    match q {
        0 => "invalid",
        1 => "gps",
        2 => "dgps",
        3 => "pps",
        4 => "rtk",
        5 => "rtk_float",
        6 => "estimated",
        7 => "manual",
        8 => "simulation",
        _ => "unknown",
    }
}

/// Verify an NMEA `*XX` checksum (XOR of every byte between `$` and `*`).
/// Returns `None` when the line carries no checksum at all.
fn checksum_ok(line: &str) -> Option<bool> {
    let start = line.find('$')?;
    let star = line.rfind('*')?;
    if star <= start {
        return None;
    }
    let body = &line[start + 1..star];
    let given = line[star + 1..].trim();
    if given.len() < 2 {
        return None;
    }
    let expected = body.bytes().fold(0u8, |a, b| a ^ b);
    let got = u8::from_str_radix(&given[..2], 16).ok()?;
    Some(expected == got)
}

/// Convert an NMEA `ddmm.mmmm` / `dddmm.mmmm` field + hemisphere into signed
/// decimal degrees. The degree width is inferred (everything left of the last
/// two integer digits is whole degrees), so it works for both lat and lon.
fn parse_coord(dm: &str, hemi: &str) -> Option<f64> {
    let dm = dm.trim();
    if dm.is_empty() {
        return None;
    }
    let dot = dm.find('.').unwrap_or(dm.len());
    if dot < 2 {
        return None;
    }
    let deg_len = dot - 2;
    let deg: f64 = if deg_len == 0 {
        0.0
    } else {
        dm.get(..deg_len)?.parse().ok()?
    };
    let min: f64 = dm.get(deg_len..)?.parse().ok()?;
    let mut val = deg + min / 60.0;
    match hemi.trim().to_ascii_uppercase().as_str() {
        "S" | "W" => val = -val,
        _ => {}
    }
    Some(val)
}

/// Format signed decimal degrees as `DD°MM'SS.ss"H`.
fn to_dms(v: f64, is_lat: bool) -> String {
    let hemi = if is_lat {
        if v < 0.0 {
            'S'
        } else {
            'N'
        }
    } else if v < 0.0 {
        'W'
    } else {
        'E'
    };
    let a = v.abs();
    let deg = a.trunc() as i64;
    let minf = (a - deg as f64) * 60.0;
    let min = minf.trunc() as i64;
    let sec = (minf - min as f64) * 60.0;
    format!("{deg}°{min:02}'{sec:05.2}\"{hemi}")
}

/// Trim the integer part of an NMEA time field (`hhmmss.ss` → `hhmmss`).
fn norm_time(s: &str) -> String {
    let s = s.trim();
    match s.find('.') {
        Some(i) => s[..i].to_string(),
        None => s.to_string(),
    }
}

/// Format `hhmmss` as `HH:MM:SS`.
fn fmt_time(hhmmss: &str) -> String {
    if hhmmss.len() < 6 {
        return hhmmss.to_string();
    }
    format!("{}:{}:{}", &hhmmss[0..2], &hhmmss[2..4], &hhmmss[4..6])
}

/// Format a full ISO-8601 UTC timestamp from a date + `hhmmss`.
fn fmt_iso(date: (i32, u32, u32), hhmmss: &str) -> String {
    let (y, m, d) = date;
    let (hh, mm, ss) = if hhmmss.len() >= 6 {
        (&hhmmss[0..2], &hhmmss[2..4], &hhmmss[4..6])
    } else {
        ("00", "00", "00")
    };
    format!("{y:04}-{m:02}-{d:02}T{hh}:{mm}:{ss}Z")
}

/// Parse an RMC `ddmmyy` date field. Two-digit years pivot at 80
/// (80..=99 → 1900s, 00..=79 → 2000s), matching the GPS era.
fn parse_date(s: &str) -> Option<(i32, u32, u32)> {
    let s = s.trim();
    if s.len() < 6 {
        return None;
    }
    let d: u32 = s.get(0..2)?.parse().ok()?;
    let m: u32 = s.get(2..4)?.parse().ok()?;
    let yy: i32 = s.get(4..6)?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let year = if yy >= 80 { 1900 + yy } else { 2000 + yy };
    Some((year, m, d))
}

/// Round to `dp` decimals then strip trailing zeros (and a bare dot).
fn fmt_num(v: f64, dp: usize) -> String {
    let s = format!("{v:.dp$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

/// RFC-4180 escape: quote a field containing the delimiter, a quote, CR or LF.
fn escape(field: &str, delim: char) -> String {
    let needs = field.contains(delim)
        || field.contains('"')
        || field.contains('\n')
        || field.contains('\r');
    if needs {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// Start (or continue) the current cycle for a sentence carrying UTC `time`.
/// A different, non-empty time flushes the current record and opens a new one.
fn cycle<'a>(cur: &'a mut Option<Record>, out: &mut Vec<Record>, time: &str) -> &'a mut Record {
    let new_cycle = match cur.as_ref() {
        None => true,
        Some(r) => !r.time.is_empty() && !time.is_empty() && r.time != time,
    };
    if new_cycle {
        if let Some(r) = cur.take() {
            out.push(r);
        }
        *cur = Some(Record::default());
    }
    let r = cur.get_or_insert_with(Record::default);
    if r.time.is_empty() && !time.is_empty() {
        r.time = time.to_string();
    }
    r
}

/// Continue the current cycle for a sentence with no time of its own.
fn current(cur: &mut Option<Record>) -> &mut Record {
    cur.get_or_insert_with(Record::default)
}

/// Parse an NMEA 0183 log into merged positional records.
fn parse(input: &str, validate_checksum: bool) -> Result<Vec<Record>, String> {
    if input.trim().is_empty() {
        return Err("no NMEA provided: paste an NMEA 0183 log — lines like $GPGGA,... or $GNRMC,...".into());
    }

    let mut out: Vec<Record> = Vec::new();
    let mut cur: Option<Record> = None;

    for raw in input.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let dollar = match line.find('$') {
            Some(i) => i,
            None => continue,
        };
        // When validation is on, a present-but-wrong checksum drops the line.
        if validate_checksum {
            if let Some(false) = checksum_ok(line) {
                continue;
            }
        }
        let body_full = &line[dollar + 1..];
        let body = match body_full.find('*') {
            Some(i) => &body_full[..i],
            None => body_full,
        };
        let fields: Vec<&str> = body.split(',').collect();
        let head = fields[0];
        if head.len() < 3 {
            continue;
        }
        // Last three chars are the sentence type; the 2-char prefix is the
        // talker (GP/GN/GL/GA/GB…) which we accept from any constellation.
        let stype = &head[head.len() - 3..];
        let get = |i: usize| fields.get(i).copied().unwrap_or("").trim();

        match stype {
            "GGA" => {
                let time = norm_time(get(1));
                let r = cycle(&mut cur, &mut out, &time);
                if r.lat.is_none() {
                    r.lat = parse_coord(get(2), get(3));
                }
                if r.lon.is_none() {
                    r.lon = parse_coord(get(4), get(5));
                }
                if r.fix.is_none() {
                    if let Ok(f) = get(6).parse::<u8>() {
                        r.fix = Some(f);
                    }
                }
                if r.satellites.is_none() {
                    if let Ok(n) = get(7).parse::<u32>() {
                        r.satellites = Some(n);
                    }
                }
                if r.hdop.is_none() && !get(8).is_empty() {
                    r.hdop = Some(get(8).to_string());
                }
                if r.altitude_m.is_none() {
                    if let Ok(a) = get(9).parse::<f64>() {
                        r.altitude_m = Some(a);
                    }
                }
            }
            "RMC" => {
                let time = norm_time(get(1));
                let r = cycle(&mut cur, &mut out, &time);
                if r.lat.is_none() {
                    r.lat = parse_coord(get(3), get(4));
                }
                if r.lon.is_none() {
                    r.lon = parse_coord(get(5), get(6));
                }
                if r.speed_knots.is_none() {
                    if let Ok(s) = get(7).parse::<f64>() {
                        r.speed_knots = Some(s);
                    }
                }
                if r.course.is_none() {
                    if let Ok(c) = get(8).parse::<f64>() {
                        r.course = Some(c);
                    }
                }
                if r.date.is_none() {
                    r.date = parse_date(get(9));
                }
            }
            "GLL" => {
                let time = norm_time(get(5));
                let r = cycle(&mut cur, &mut out, &time);
                if r.lat.is_none() {
                    r.lat = parse_coord(get(1), get(2));
                }
                if r.lon.is_none() {
                    r.lon = parse_coord(get(3), get(4));
                }
            }
            "VTG" => {
                let r = current(&mut cur);
                if r.course.is_none() {
                    if let Ok(c) = get(1).parse::<f64>() {
                        r.course = Some(c);
                    }
                }
                if r.speed_knots.is_none() {
                    if let Ok(s) = get(5).parse::<f64>() {
                        r.speed_knots = Some(s);
                    }
                }
            }
            "ZDA" => {
                let time = norm_time(get(1));
                let r = cycle(&mut cur, &mut out, &time);
                if let (Ok(d), Ok(m), Ok(y)) = (
                    get(2).parse::<u32>(),
                    get(3).parse::<u32>(),
                    get(4).parse::<i32>(),
                ) {
                    if (1..=12).contains(&m) && (1..=31).contains(&d) {
                        r.date = Some((y, m, d));
                    }
                }
            }
            "GSA" => {
                let r = current(&mut cur);
                if r.hdop.is_none() && !get(16).is_empty() {
                    r.hdop = Some(get(16).to_string());
                }
            }
            _ => {}
        }
    }

    if let Some(r) = cur.take() {
        out.push(r);
    }

    // Keep only fixes that carry a position.
    out.retain(|r| r.has_position());

    // Propagate the date across cycles: forward-fill, then back-fill any
    // leading records from the first known date.
    let mut last: Option<(i32, u32, u32)> = None;
    for r in out.iter_mut() {
        match r.date {
            Some(d) => last = Some(d),
            None => r.date = last,
        }
    }
    if let Some(first) = out.iter().find_map(|r| r.date) {
        for r in out.iter_mut() {
            if r.date.is_none() {
                r.date = Some(first);
            }
        }
    }

    Ok(out)
}

/// Convert an NMEA 0183 log to CSV. See the module docs / descriptor for the
/// column set and parameter semantics.
#[allow(clippy::too_many_arguments)]
pub fn convert(
    input: &str,
    coordinates: Coordinates,
    altitude_unit: AltitudeUnit,
    speed_unit: SpeedUnit,
    delimiter: Delimiter,
    header: bool,
    validate_checksum: bool,
) -> Result<String, String> {
    let records = parse(input, validate_checksum)?;
    if records.is_empty() {
        return Err(
            "no positional fixes found: the log has no GGA/RMC/GLL sentences with valid coordinates (if you enabled checksum validation, every line may have failed it)"
                .into(),
        );
    }

    let has_date = records.iter().any(|r| r.date.is_some());
    let delim = delimiter.ch();

    let mut cols: Vec<String> = Vec::new();
    if has_date {
        cols.push("timestamp".to_string());
    }
    cols.push("time".to_string());
    cols.push("latitude".to_string());
    cols.push("longitude".to_string());
    cols.push(format!("altitude_{}", altitude_unit.suffix()));
    cols.push("fix".to_string());
    cols.push("satellites".to_string());
    cols.push("hdop".to_string());
    cols.push(format!("speed_{}", speed_unit.suffix()));
    cols.push("course".to_string());

    let mut lines: Vec<String> = Vec::new();
    if header {
        let row: Vec<String> = cols.iter().map(|c| escape(c, delim)).collect();
        lines.push(row.join(&delim.to_string()));
    }

    for r in &records {
        let mut f: Vec<String> = Vec::with_capacity(cols.len());
        if has_date {
            f.push(match r.date {
                Some(d) => fmt_iso(d, &r.time),
                None => String::new(),
            });
        }
        f.push(fmt_time(&r.time));
        let (lat_s, lon_s) = match coordinates {
            Coordinates::Decimal => (
                r.lat.map(|v| fmt_num(v, 6)).unwrap_or_default(),
                r.lon.map(|v| fmt_num(v, 6)).unwrap_or_default(),
            ),
            Coordinates::Dms => (
                r.lat.map(|v| to_dms(v, true)).unwrap_or_default(),
                r.lon.map(|v| to_dms(v, false)).unwrap_or_default(),
            ),
        };
        f.push(lat_s);
        f.push(lon_s);
        f.push(
            r.altitude_m
                .map(|m| fmt_num(altitude_unit.from_meters(m), 2))
                .unwrap_or_default(),
        );
        f.push(r.fix.map(|q| fix_label(q).to_string()).unwrap_or_default());
        f.push(r.satellites.map(|n| n.to_string()).unwrap_or_default());
        f.push(r.hdop.clone().unwrap_or_default());
        f.push(
            r.speed_knots
                .map(|kn| fmt_num(speed_unit.from_knots(kn), 2))
                .unwrap_or_default(),
        );
        f.push(r.course.map(|c| fmt_num(c, 1)).unwrap_or_default());

        let row: Vec<String> = f.iter().map(|x| escape(x, delim)).collect();
        lines.push(row.join(&delim.to_string()));
    }

    Ok(lines.join("\n"))
}

/// String-argument convenience used by the chat block, CLI, and web wrapper.
#[allow(clippy::too_many_arguments)]
pub fn convert_str(
    input: &str,
    coordinates: &str,
    altitude_unit: &str,
    speed_unit: &str,
    delimiter: &str,
    header: bool,
    validate_checksum: bool,
) -> Result<String, String> {
    convert(
        input,
        Coordinates::parse(coordinates)?,
        AltitudeUnit::parse(altitude_unit)?,
        SpeedUnit::parse(speed_unit)?,
        Delimiter::parse(delimiter)?,
        header,
        validate_checksum,
    )
}

/// Back-compat single-string entry point (defaults for every option).
pub fn run(input: &str) -> Result<String, String> {
    convert_str(input, "decimal", "m", "knots", "comma", true, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Textbook cycle with correct checksums (validated by an external tool).
    const GGA: &str = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47";
    const RMC: &str = "$GPRMC,123519,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W*6A";
    const VTG: &str = "$GPVTG,084.4,T,,M,022.4,N,041.5,K*6C";

    fn conv(
        input: &str,
        coord: &str,
        alt: &str,
        speed: &str,
        delim: &str,
        header: bool,
        validate: bool,
    ) -> String {
        convert_str(input, coord, alt, speed, delim, header, validate).unwrap()
    }

    #[test]
    fn happy_mixed_gga_rmc_vtg_merge_one_row() {
        let input = format!("{GGA}\n{RMC}\n{VTG}");
        let out = conv(&input, "decimal", "m", "knots", "comma", true, false);
        assert_eq!(
            out,
            "timestamp,time,latitude,longitude,altitude_m,fix,satellites,hdop,speed_knots,course\n\
             1994-03-23T12:35:19Z,12:35:19,48.1173,11.516667,545.4,gps,8,0.9,22.4,84.4"
        );
        // GGA + RMC + VTG share time 123519 → exactly one data row.
        assert_eq!(out.lines().count(), 2);
    }

    #[test]
    fn two_cycles_become_two_rows() {
        let c2_gga = "$GPGGA,123520,4807.040,N,01131.010,E,1,07,1.0,546.0,M,46.9,M,,";
        let input = format!("{GGA}\n{RMC}\n{c2_gga}");
        let out = conv(&input, "decimal", "m", "knots", "comma", false, false);
        assert_eq!(out.lines().count(), 2, "distinct times → distinct rows: {out}");
        assert!(out
            .lines()
            .nth(1)
            .unwrap()
            .starts_with("1994-03-23T12:35:20Z,12:35:20"));
    }

    #[test]
    fn checksum_validate_skips_bad_line_but_keeps_good() {
        // GGA checksum deliberately wrong (*00), RMC correct.
        let bad_gga = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*00";
        let input = format!("{bad_gga}\n{RMC}");

        // validate=true → GGA dropped, so altitude/fix/satellites are blank but
        // the RMC-derived row (position + speed + course + date) survives.
        let strict = conv(&input, "decimal", "m", "knots", "comma", true, true);
        assert_eq!(strict.lines().count(), 2);
        assert!(!strict.contains("545.4"), "GGA altitude must be skipped: {strict}");
        assert!(!strict.contains("gps"), "GGA fix must be skipped: {strict}");
        assert!(strict.contains(",22.4,84.4"), "RMC speed/course kept: {strict}");

        // validate=false → the bad-checksum GGA is parsed too, altitude returns.
        let loose = conv(&input, "decimal", "m", "knots", "comma", true, false);
        assert!(loose.contains("545.4"), "GGA altitude kept when not validating: {loose}");
    }

    #[test]
    fn checksum_validate_all_bad_errors() {
        let bad_gga = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*00";
        let bad_rmc = "$GPRMC,123519,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W*00";
        let input = format!("{bad_gga}\n{bad_rmc}");
        let err = convert_str(&input, "decimal", "m", "knots", "comma", true, true).unwrap_err();
        assert!(err.contains("no positional fixes"), "{err}");
    }

    #[test]
    fn coordinates_dms_format() {
        let out = conv(GGA, "dms", "m", "knots", "comma", false, false);
        // The seconds marker (") makes each DMS field RFC-4180 quoted (interior
        // quote doubled): 48°07'02.28"N → "48°07'02.28""N".
        assert!(out.contains("\"48°07'02.28\"\"N\""), "{out}");
        assert!(out.contains("\"11°31'00.00\"\"E\""), "{out}");
    }

    #[test]
    fn altitude_unit_feet() {
        let out = conv(GGA, "decimal", "ft", "knots", "comma", true, false);
        assert!(out.lines().next().unwrap().contains("altitude_ft"));
        // 545.4 m * 3.28084 ≈ 1789.37 ft
        assert!(out.contains("1789.37"), "{out}");
    }

    #[test]
    fn speed_units_knots_kmh_mph() {
        let kn = conv(RMC, "decimal", "m", "knots", "comma", true, false);
        assert!(kn.lines().next().unwrap().contains("speed_knots") && kn.contains(",22.4,"));
        let kmh = conv(RMC, "decimal", "m", "kmh", "comma", true, false);
        // 22.4 kn * 1.852 = 41.4848 → 41.48
        assert!(
            kmh.lines().next().unwrap().contains("speed_kmh") && kmh.contains(",41.48,"),
            "{kmh}"
        );
        let mph = conv(RMC, "decimal", "m", "mph", "comma", true, false);
        // 22.4 kn * 1.150779 = 25.777 → 25.78
        assert!(
            mph.lines().next().unwrap().contains("speed_mph") && mph.contains(",25.78,"),
            "{mph}"
        );
    }

    #[test]
    fn delimiters_semicolon_tab_pipe() {
        assert!(conv(GGA, "decimal", "m", "knots", "semicolon", true, false).contains("time;latitude;"));
        assert!(conv(GGA, "decimal", "m", "knots", "tab", true, false).contains("time\tlatitude\t"));
        assert!(conv(GGA, "decimal", "m", "knots", "pipe", true, false).contains("time|latitude|"));
    }

    #[test]
    fn header_false_drops_header_row() {
        let out = conv(GGA, "decimal", "m", "knots", "comma", false, false);
        assert!(!out.starts_with("time"));
        assert_eq!(out.lines().count(), 1);
    }

    #[test]
    fn gga_only_has_no_timestamp_column() {
        // No RMC/ZDA → no date → the timestamp column is omitted entirely.
        let out = conv(GGA, "decimal", "m", "knots", "comma", true, false);
        let header = out.lines().next().unwrap();
        assert!(!header.contains("timestamp"), "{header}");
        assert!(header.starts_with("time,latitude"), "{header}");
    }

    #[test]
    fn zda_supplies_date_and_gll_position() {
        let zda = "$GPZDA,225444.00,04,07,2002,00,00*64";
        let gll = "$GPGLL,4916.45,N,12311.12,W,225444,A*31";
        let input = format!("{zda}\n{gll}");
        let out = conv(&input, "decimal", "m", "knots", "comma", false, true);
        // date 2002-07-04 from ZDA, position from GLL (west → negative lon).
        assert!(out.starts_with("2002-07-04T22:54:44Z,22:54:44,49.2741"), "{out}");
        assert!(out.contains(",-123."), "west longitude negative: {out}");
    }

    #[test]
    fn gsa_backfills_hdop_when_no_gga() {
        // RMC (no HDOP) + GSA (HDOP 1.3) in one cycle.
        let gsa = "$GPGSA,A,3,04,05,,09,12,,,24,,,,,2.5,1.3,2.1*39";
        let input = format!("{RMC}\n{gsa}");
        let out = conv(&input, "decimal", "m", "knots", "comma", true, false);
        assert!(out.lines().next().unwrap().contains(",hdop,"));
        assert!(out.contains(",1.3,"), "GSA HDOP backfilled: {out}");
    }

    #[test]
    fn enum_parse_errors() {
        assert!(Coordinates::parse("polar").is_err());
        assert!(AltitudeUnit::parse("cubits").is_err());
        assert!(SpeedUnit::parse("furlongs").is_err());
        assert!(Delimiter::parse("colon").is_err());
        // aliases accepted
        assert_eq!(Coordinates::parse("dd").unwrap(), Coordinates::Decimal);
        assert_eq!(SpeedUnit::parse("km/h").unwrap(), SpeedUnit::Kmh);
    }

    #[test]
    fn empty_and_nonpositional_errors() {
        assert!(convert_str("", "decimal", "m", "knots", "comma", true, false)
            .unwrap_err()
            .contains("no NMEA provided"));
        // A VTG-only log has speed but no position → no rows.
        assert!(convert_str(VTG, "decimal", "m", "knots", "comma", true, false)
            .unwrap_err()
            .contains("no positional fixes"));
    }

    #[test]
    fn gnss_talker_prefixes_accepted() {
        let gn = "$GNGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,";
        let out = conv(gn, "decimal", "m", "knots", "comma", false, false);
        assert!(out.starts_with("12:35:19,48.1173"), "{out}");
    }

    #[test]
    fn rfc4180_no_spurious_quoting_on_tab() {
        // Fields here never contain a delimiter, so nothing should be quoted.
        let out = conv(GGA, "decimal", "m", "knots", "tab", false, false);
        assert!(!out.contains('"'));
    }
}
