//! Reproject a CSV of UTM easting/northing/zone coordinates to WGS84-style
//! latitude/longitude, and back again.
//!
//! The engine is a closed-form Transverse Mercator (Snyder, USGS Professional
//! Paper 1395) evaluated on a selectable ellipsoid, with the UTM scale factor
//! `k0 = 0.9996`, a 500 km false easting and a 10 000 km false northing in the
//! southern hemisphere. It is header-aware: it picks the easting, northing and
//! zone columns by name or 1-based index (auto-detecting the usual spellings
//! when none is given), keeps every other column intact, and re-emits the table
//! as CSV, TSV, JSON, an aligned text table, GeoJSON or KML.
//!
//! Changing the ellipsoid changes the shape of the projection surface. It does
//! **not** apply a datum shift — see the crate's page copy.

use std::f64::consts::{PI, TAU};

/// Largest accepted input size, in bytes.
pub const MAX_INPUT_BYTES: usize = 5 * 1024 * 1024;
/// Largest accepted number of data rows.
pub const MAX_ROWS: usize = 200_000;
/// Largest accepted `decimals` value.
pub const MAX_DECIMALS: usize = 12;
/// UTM's scale factor on the central meridian.
const K0: f64 = 0.9996;
/// UTM false easting, in metres.
const FALSE_EASTING: f64 = 500_000.0;
/// UTM false northing applied in the southern hemisphere, in metres.
const FALSE_NORTHING: f64 = 10_000_000.0;
/// MGRS latitude-band letters, from 80°S upward in 8° steps (`X` spans 12°).
const BAND_LETTERS: &[u8] = b"CDEFGHJKLMNPQRSTUVWX";

// ---------------------------------------------------------------------------
// Projection
// ---------------------------------------------------------------------------

/// A Transverse Mercator projection on one ellipsoid, with the meridian-arc and
/// footpoint-latitude series precomputed from the flattening.
struct Tm {
    a: f64,
    e2: f64,
    ep2: f64,
    m1: f64,
    m2: f64,
    m3: f64,
    m4: f64,
    p2: f64,
    p3: f64,
    p4: f64,
    p5: f64,
}

impl Tm {
    /// Build from a semi-major axis and inverse flattening.
    fn new(a: f64, inv_f: f64) -> Self {
        let f = 1.0 / inv_f;
        let e2 = f * (2.0 - f);
        let (e4, e6) = (e2 * e2, e2 * e2 * e2);
        let sqrt_e = (1.0 - e2).sqrt();
        let n = (1.0 - sqrt_e) / (1.0 + sqrt_e);
        let (n2, n3, n4, n5) = (n * n, n * n * n, n * n * n * n, n * n * n * n * n);
        Tm {
            a,
            e2,
            ep2: e2 / (1.0 - e2),
            m1: 1.0 - e2 / 4.0 - 3.0 * e4 / 64.0 - 5.0 * e6 / 256.0,
            m2: 3.0 * e2 / 8.0 + 3.0 * e4 / 32.0 + 45.0 * e6 / 1024.0,
            m3: 15.0 * e4 / 256.0 + 45.0 * e6 / 1024.0,
            m4: 35.0 * e6 / 3072.0,
            p2: 3.0 / 2.0 * n - 27.0 / 32.0 * n3 + 269.0 / 512.0 * n5,
            p3: 21.0 / 16.0 * n2 - 55.0 / 32.0 * n4,
            p4: 151.0 / 96.0 * n3 - 417.0 / 128.0 * n5,
            p5: 1097.0 / 512.0 * n4,
        }
    }

    /// UTM easting/northing → latitude/longitude, in degrees.
    fn to_latlon(&self, easting: f64, northing: f64, zone: u8, northern: bool) -> (f64, f64) {
        let x = easting - FALSE_EASTING;
        let y = if northern {
            northing
        } else {
            northing - FALSE_NORTHING
        };

        // Footpoint latitude from the meridional arc.
        let mu = y / K0 / (self.a * self.m1);
        let foot = mu
            + self.p2 * (2.0 * mu).sin()
            + self.p3 * (4.0 * mu).sin()
            + self.p4 * (6.0 * mu).sin()
            + self.p5 * (8.0 * mu).sin();

        let (sin_f, cos_f) = (foot.sin(), foot.cos());
        let tan_f = sin_f / cos_f;
        let (t, t2) = (tan_f * tan_f, tan_f * tan_f * tan_f * tan_f);
        let w = 1.0 - self.e2 * sin_f * sin_f;
        let n = self.a / w.sqrt();
        // N1/R1, the ratio of the normal and meridional radii at the footpoint.
        let ratio = w / (1.0 - self.e2);
        let c = self.ep2 * cos_f * cos_f;
        let c2 = c * c;

        let d = x / (n * K0);
        let (d2, d3) = (d * d, d * d * d);
        let (d4, d5, d6) = (d2 * d2, d2 * d3, d3 * d3);

        let lat = foot
            - tan_f
                * ratio
                * (d2 / 2.0 - d4 / 24.0 * (5.0 + 3.0 * t + 10.0 * c - 4.0 * c2 - 9.0 * self.ep2)
                    + d6 / 720.0
                        * (61.0 + 90.0 * t + 298.0 * c + 45.0 * t2 - 252.0 * self.ep2 - 3.0 * c2));
        let lon = (d - d3 / 6.0 * (1.0 + 2.0 * t + c)
            + d5 / 120.0
                * (5.0 - 2.0 * c + 28.0 * t - 3.0 * c2 + 8.0 * self.ep2 + 24.0 * t2))
            / cos_f;

        (lat.to_degrees(), lon.to_degrees() + central_meridian(zone))
    }

    /// Latitude/longitude in degrees → UTM easting/northing in the given zone.
    fn from_latlon(&self, lat_deg: f64, lon_deg: f64, zone: u8) -> (f64, f64) {
        let lat = lat_deg.to_radians();
        let (sin_l, cos_l) = (lat.sin(), lat.cos());
        let tan_l = sin_l / cos_l;
        let (t, t2) = (tan_l * tan_l, tan_l * tan_l * tan_l * tan_l);

        let n = self.a / (1.0 - self.e2 * sin_l * sin_l).sqrt();
        let c = self.ep2 * cos_l * cos_l;
        let offset = cos_l * wrap_pi(lon_deg.to_radians() - central_meridian(zone).to_radians());
        let (o2, o3) = (offset * offset, offset * offset * offset);
        let (o4, o5, o6) = (o2 * o2, o2 * o3, o3 * o3);

        let m = self.a
            * (self.m1 * lat - self.m2 * (2.0 * lat).sin() + self.m3 * (4.0 * lat).sin()
                - self.m4 * (6.0 * lat).sin());

        let easting = K0
            * n
            * (offset
                + o3 / 6.0 * (1.0 - t + c)
                + o5 / 120.0 * (5.0 - 18.0 * t + t2 + 72.0 * c - 58.0 * self.ep2))
            + FALSE_EASTING;
        let mut northing = K0
            * (m + n
                * tan_l
                * (o2 / 2.0
                    + o4 / 24.0 * (5.0 - t + 9.0 * c + 4.0 * c * c)
                    + o6 / 720.0 * (61.0 - 58.0 * t + t2 + 600.0 * c - 330.0 * self.ep2)));
        if lat_deg < 0.0 {
            northing += FALSE_NORTHING;
        }
        (easting, northing)
    }
}

/// Central meridian of a UTM zone, in degrees.
fn central_meridian(zone: u8) -> f64 {
    (zone as f64 - 1.0) * 6.0 - 180.0 + 3.0
}

/// Wrap an angle in radians into `(-π, π]`.
fn wrap_pi(v: f64) -> f64 {
    (v + PI).rem_euclid(TAU) - PI
}

/// The UTM zone a longitude falls in, including the widened Norway zone 32V and
/// the four Svalbard zones (31X/33X/35X/37X).
fn zone_for(lat: f64, lon: f64) -> u8 {
    if (56.0..64.0).contains(&lat) && (3.0..12.0).contains(&lon) {
        return 32;
    }
    if (72.0..=84.0).contains(&lat) && lon >= 0.0 {
        if lon < 9.0 {
            return 31;
        } else if lon < 21.0 {
            return 33;
        } else if lon < 33.0 {
            return 35;
        } else if lon < 42.0 {
            return 37;
        }
    }
    let n = ((lon + 180.0) / 6.0).floor() as i64 + 1;
    n.clamp(1, 60) as u8
}

/// The MGRS latitude band a latitude falls in, or `None` outside UTM's range.
fn band_for(lat: f64) -> Option<char> {
    if !(-80.0..=84.0).contains(&lat) {
        return None;
    }
    let idx = (((lat + 80.0) / 8.0).floor() as usize).min(BAND_LETTERS.len() - 1);
    Some(BAND_LETTERS[idx] as char)
}

/// Latitude span covered by a band letter, or `None` if it is not a band letter.
fn band_range(letter: char) -> Option<(f64, f64)> {
    let idx = BAND_LETTERS
        .iter()
        .position(|b| *b == letter.to_ascii_uppercase() as u8)?;
    let low = -80.0 + 8.0 * idx as f64;
    // X is the only 12°-tall band; it runs to 84°N.
    Some((low, if letter.eq_ignore_ascii_case(&'X') { 84.0 } else { low + 8.0 }))
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Direction {
    UtmToLatLon,
    LatLonToUtm,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Hemisphere {
    Auto,
    North,
    South,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CoordFormat {
    Decimal,
    Dms,
    Ddm,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Output {
    Csv,
    Tsv,
    Json,
    Table,
    GeoJson,
    Kml,
}

/// A zone as written by the user: a number plus whatever hemisphere evidence
/// came with it (a latitude-band letter, or an EPSG code).
#[derive(Clone, Copy, Debug)]
struct ZoneSpec {
    number: u8,
    band: Option<char>,
    /// `Some(true)` = northern, from an EPSG code or a band letter.
    northern: Option<bool>,
}

/// Parse a zone written as `18`, `18T`, `18 N`, `zone 18`, `EPSG:32618` or the
/// bare EPSG codes `32618` (north) / `32718` (south).
fn parse_zone(spec: &str) -> Result<ZoneSpec, String> {
    let cleaned = spec.trim().to_ascii_uppercase();
    let cleaned = cleaned
        .strip_prefix("EPSG:")
        .or_else(|| cleaned.strip_prefix("EPSG "))
        .or_else(|| cleaned.strip_prefix("ZONE "))
        .unwrap_or(&cleaned)
        .trim();
    if cleaned.is_empty() {
        return Err("zone is empty".to_string());
    }

    let digits: String = cleaned.chars().take_while(|c| c.is_ascii_digit()).collect();
    let rest: String = cleaned
        .chars()
        .skip(digits.len())
        .filter(|c| !c.is_whitespace() && *c != '-' && *c != '_')
        .collect();
    if digits.is_empty() {
        return Err(format!(
            "zone \"{spec}\" has no zone number — write it like 18, 18T or EPSG:32618"
        ));
    }
    let number: u32 = digits
        .parse()
        .map_err(|_| format!("zone \"{spec}\" is not a number"))?;

    // EPSG 326xx = WGS84 / UTM north, 327xx = south.
    if (32601..=32660).contains(&number) || (32701..=32760).contains(&number) {
        if !rest.is_empty() {
            return Err(format!(
                "zone \"{spec}\" mixes an EPSG code with a band letter — use one or the other"
            ));
        }
        let northern = number < 32700;
        return Ok(ZoneSpec {
            number: (number % 100) as u8,
            band: None,
            northern: Some(northern),
        });
    }
    if !(1..=60).contains(&number) {
        return Err(format!(
            "zone \"{spec}\" is out of range — UTM zones run 1 to 60 (or use an EPSG code like 32618)"
        ));
    }

    match rest.len() {
        0 => Ok(ZoneSpec {
            number: number as u8,
            band: None,
            northern: None,
        }),
        1 => {
            let letter = rest.chars().next().expect("length checked");
            // A bare N or S is read the way GIS software names its projections
            // ("UTM zone 56S" = zone 56, southern hemisphere), not as MGRS band
            // N (0-8°N) or band S (32-40°N). Every other letter is a band.
            if letter == 'N' || letter == 'S' {
                return Ok(ZoneSpec {
                    number: number as u8,
                    band: None,
                    northern: Some(letter == 'N'),
                });
            }
            if band_range(letter).is_none() {
                return Err(format!(
                    "zone \"{spec}\" ends in \"{letter}\", which is not a UTM latitude band — \
                     bands run C to X, skipping I and O"
                ));
            }
            Ok(ZoneSpec {
                number: number as u8,
                band: Some(letter),
                // Bands N through X are north of the equator, C through M south.
                northern: Some(letter >= 'N'),
            })
        }
        _ => Err(format!(
            "zone \"{spec}\" has trailing characters — write it like 18, 18T or EPSG:32618"
        )),
    }
}

/// Parsed, validated options for one conversion run.
struct Options {
    direction: Direction,
    hemisphere: Hemisphere,
    coord_format: CoordFormat,
    decimals: usize,
    delimiter: u8,
    has_header: bool,
    keep_columns: bool,
    validate_ranges: bool,
    output: Output,
    tm: Tm,
}

/// Column names the auto-detector accepts for each slot.
const EASTING_ALIASES: &[&str] = &["easting", "east", "utm_easting", "utm easting", "e", "x"];
const NORTHING_ALIASES: &[&str] = &["northing", "north", "utm_northing", "utm northing", "n", "y"];
const ZONE_ALIASES: &[&str] = &["zone", "utm_zone", "utm zone", "zone_number", "zonenumber", "utmzone"];
const LAT_ALIASES: &[&str] = &["latitude", "lat", "y", "lat_dd", "latitude_dd"];
const LON_ALIASES: &[&str] = &["longitude", "lon", "lng", "long", "x", "lon_dd", "longitude_dd"];

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Reproject `csv_text` between UTM and latitude/longitude.
///
/// `easting_column` / `northing_column` name the two coordinate columns (header
/// name or 1-based index) — the easting and northing when going UTM → lat/lon,
/// the latitude and longitude when going the other way. Empty strings
/// auto-detect. Returns the rendered table, or a human-readable error naming the
/// offending row.
#[allow(clippy::too_many_arguments)]
pub fn convert(
    csv_text: &str,
    direction: &str,
    easting_column: &str,
    northing_column: &str,
    zone_column: &str,
    zone: &str,
    hemisphere: &str,
    coord_format: &str,
    decimals: i64,
    ellipsoid: &str,
    delimiter: &str,
    has_header: bool,
    keep_columns: bool,
    validate_ranges: bool,
    output: &str,
) -> Result<String, String> {
    if csv_text.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "CSV input is {} bytes, above the {MAX_INPUT_BYTES} byte limit",
            csv_text.len()
        ));
    }
    if csv_text.trim().is_empty() {
        return Err("CSV input is empty — paste at least one row of coordinates".to_string());
    }

    let direction = match direction.trim().to_ascii_lowercase().as_str() {
        "" | "utm_to_latlon" | "to_latlon" | "utm_to_wgs84" => Direction::UtmToLatLon,
        "latlon_to_utm" | "to_utm" | "wgs84_to_utm" => Direction::LatLonToUtm,
        other => {
            return Err(format!(
                "unknown direction \"{other}\" — use utm_to_latlon or latlon_to_utm"
            ))
        }
    };
    let hemisphere = match hemisphere.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Hemisphere::Auto,
        "north" | "n" | "northern" => Hemisphere::North,
        "south" | "s" | "southern" => Hemisphere::South,
        other => {
            return Err(format!(
                "unknown hemisphere \"{other}\" — use auto, north or south"
            ))
        }
    };
    let coord_format = match coord_format.trim().to_ascii_lowercase().as_str() {
        "" | "decimal" | "dd" => CoordFormat::Decimal,
        "dms" => CoordFormat::Dms,
        "ddm" => CoordFormat::Ddm,
        other => {
            return Err(format!(
                "unknown coord_format \"{other}\" — use decimal, dms or ddm"
            ))
        }
    };
    if !(0..=MAX_DECIMALS as i64).contains(&decimals) {
        return Err(format!(
            "decimals must be between 0 and {MAX_DECIMALS}, got {decimals}"
        ));
    }
    let tm = match ellipsoid.trim().to_ascii_lowercase().replace([' ', '-', '_'], "").as_str() {
        "" | "wgs84" => Tm::new(6_378_137.0, 298.257_223_563),
        "grs80" => Tm::new(6_378_137.0, 298.257_222_101),
        "clarke1866" => Tm::new(6_378_206.4, 294.978_698_2),
        "international1924" | "intl1924" | "hayford" => Tm::new(6_378_388.0, 297.0),
        other => {
            return Err(format!(
                "unknown ellipsoid \"{other}\" — use wgs84, grs80, clarke1866 or international1924"
            ))
        }
    };
    let output = match output.trim().to_ascii_lowercase().as_str() {
        "" | "csv" => Output::Csv,
        "tsv" => Output::Tsv,
        "json" => Output::Json,
        "table" => Output::Table,
        "geojson" => Output::GeoJson,
        "kml" => Output::Kml,
        other => {
            return Err(format!(
                "unknown output \"{other}\" — use csv, tsv, json, table, geojson or kml"
            ))
        }
    };
    let delimiter = resolve_delimiter(delimiter, csv_text)?;
    let fallback_zone = if zone.trim().is_empty() {
        None
    } else {
        Some(parse_zone(zone)?)
    };

    let opts = Options {
        direction,
        hemisphere,
        coord_format,
        decimals: decimals as usize,
        delimiter,
        has_header,
        keep_columns,
        validate_ranges,
        output,
        tm,
    };

    let records = read_records(csv_text, opts.delimiter)?;
    if records.is_empty() {
        return Err("CSV input is empty — paste at least one row of coordinates".to_string());
    }

    let (headers, rows) = if opts.has_header {
        let (head, rest) = records.split_first().expect("records is non-empty");
        (head.clone(), rest.to_vec())
    } else {
        let width = records.iter().map(Vec::len).max().unwrap_or(0);
        let head = (1..=width).map(|i| format!("column{i}")).collect::<Vec<_>>();
        (head, records)
    };
    if rows.is_empty() {
        return Err(
            "CSV has a header but no data rows — add at least one row of coordinates".to_string(),
        );
    }
    if rows.len() > MAX_ROWS {
        return Err(format!(
            "CSV has {} data rows, above the {MAX_ROWS} row limit",
            rows.len()
        ));
    }

    let (first_aliases, second_aliases) = match opts.direction {
        Direction::UtmToLatLon => (EASTING_ALIASES, NORTHING_ALIASES),
        Direction::LatLonToUtm => (LAT_ALIASES, LON_ALIASES),
    };
    let (first_label, second_label) = match opts.direction {
        Direction::UtmToLatLon => ("easting", "northing"),
        Direction::LatLonToUtm => ("latitude", "longitude"),
    };
    let first_idx = resolve_column(
        easting_column,
        &headers,
        first_aliases,
        "easting_column",
        first_label,
        0,
    )?;
    let second_idx = resolve_column(
        northing_column,
        &headers,
        second_aliases,
        "northing_column",
        second_label,
        1,
    )?;
    if first_idx == second_idx {
        return Err(format!(
            "easting_column and northing_column both resolve to column {} — pick two different columns",
            first_idx + 1
        ));
    }
    let zone_idx = resolve_zone_column(zone_column, &headers)?;
    if zone_idx == Some(first_idx) || zone_idx == Some(second_idx) {
        return Err(format!(
            "zone_column resolves to column {}, which is already a coordinate column",
            zone_idx.expect("matched above") + 1
        ));
    }
    if opts.direction == Direction::UtmToLatLon && zone_idx.is_none() && fallback_zone.is_none() {
        return Err(
            "no UTM zone found — add a zone column to the CSV, name it with zone_column, or set \
             the zone parameter (for example 18T or EPSG:32618)"
                .to_string(),
        );
    }

    let out_names: &[&str] = match opts.direction {
        Direction::UtmToLatLon => &["latitude", "longitude"],
        Direction::LatLonToUtm => &["easting", "northing", "zone", "hemisphere"],
    };
    let mut out_headers: Vec<String> = Vec::new();
    let mut kept: Vec<usize> = Vec::new();
    if opts.keep_columns {
        for (i, h) in headers.iter().enumerate() {
            if i != first_idx && i != second_idx && Some(i) != zone_idx {
                kept.push(i);
                out_headers.push(h.clone());
            }
        }
    }
    out_headers.extend(out_names.iter().map(|s| s.to_string()));

    let mut out_rows: Vec<Vec<String>> = Vec::with_capacity(rows.len());
    // Decimal lon/lat per row, for the GeoJSON and KML geometries.
    let mut points: Vec<(f64, f64)> = Vec::with_capacity(rows.len());

    for (n, row) in rows.iter().enumerate() {
        let line_no = n + 1 + usize::from(opts.has_header);
        let first_raw = cell(row, first_idx, line_no, &headers)?;
        let second_raw = cell(row, second_idx, line_no, &headers)?;

        let (converted, lon_lat) = match opts.direction {
            Direction::UtmToLatLon => {
                let easting = parse_number(first_raw, line_no, &headers, first_idx)?;
                let northing = parse_number(second_raw, line_no, &headers, second_idx)?;
                let spec = row_zone(row, zone_idx, &fallback_zone, line_no)?;
                let northern = match opts.hemisphere {
                    Hemisphere::North => true,
                    Hemisphere::South => false,
                    Hemisphere::Auto => spec.northern.unwrap_or(true),
                };
                if opts.validate_ranges {
                    check_utm_ranges(easting, northing, line_no)?;
                }
                let (lat, lon) = opts.tm.to_latlon(easting, northing, spec.number, northern);
                if opts.validate_ranges && opts.hemisphere == Hemisphere::Auto {
                    check_band(spec, lat, line_no)?;
                }
                (
                    vec![
                        format_latitude(lat, &opts),
                        format_longitude(lon, &opts),
                    ],
                    (lon, lat),
                )
            }
            Direction::LatLonToUtm => {
                let lat = parse_angle(first_raw, line_no, &headers, first_idx, true)?;
                let lon = parse_angle(second_raw, line_no, &headers, second_idx, false)?;
                if opts.validate_ranges && !(-80.0..=84.0).contains(&lat) {
                    return Err(format!(
                        "row {line_no}: latitude {lat} is outside UTM's 80°S to 84°N range — \
                         the poles use the UPS grid, which this tool does not produce"
                    ));
                }
                let forced = row_zone_optional(row, zone_idx, &fallback_zone, line_no)?;
                let number = match forced {
                    Some(spec) => spec.number,
                    None => zone_for(lat, lon),
                };
                let (easting, northing) = opts.tm.from_latlon(lat, lon, number);
                let band = band_for(lat);
                let zone_text = match band {
                    Some(letter) => format!("{number}{letter}"),
                    None => format!("{number}"),
                };
                (
                    vec![
                        format_number(easting, opts.decimals),
                        format_number(northing, opts.decimals),
                        zone_text,
                        if lat < 0.0 { "S".into() } else { "N".into() },
                    ],
                    (lon, lat),
                )
            }
        };

        let mut out_row: Vec<String> = Vec::with_capacity(out_headers.len());
        for i in &kept {
            out_row.push(row.get(*i).cloned().unwrap_or_default());
        }
        out_row.extend(converted);
        out_rows.push(out_row);
        points.push(lon_lat);
    }

    Ok(render(&out_headers, &out_rows, &points, &opts))
}

/// Default page/CLI conversion: headered UTM CSV to latitude/longitude CSV.
pub fn run(csv_text: &str) -> Result<String, String> {
    convert(
        csv_text,
        "utm_to_latlon",
        "",
        "",
        "",
        "",
        "auto",
        "decimal",
        6,
        "wgs84",
        "auto",
        true,
        true,
        true,
        "csv",
    )
}

/// Resolve the zone for one row: the row's own zone cell wins, the `zone`
/// parameter fills in for rows without one.
fn row_zone(
    row: &[String],
    zone_idx: Option<usize>,
    fallback: &Option<ZoneSpec>,
    line_no: usize,
) -> Result<ZoneSpec, String> {
    row_zone_optional(row, zone_idx, fallback, line_no)?.ok_or_else(|| {
        format!("row {line_no}: the zone cell is empty and no zone parameter was given")
    })
}

/// As [`row_zone`], but `None` (rather than an error) when no zone is available.
fn row_zone_optional(
    row: &[String],
    zone_idx: Option<usize>,
    fallback: &Option<ZoneSpec>,
    line_no: usize,
) -> Result<Option<ZoneSpec>, String> {
    if let Some(idx) = zone_idx {
        if let Some(raw) = row.get(idx) {
            if !raw.trim().is_empty() {
                return parse_zone(raw)
                    .map(Some)
                    .map_err(|e| format!("row {line_no}: {e}"));
            }
        }
    }
    Ok(*fallback)
}

/// Bounds-check a UTM pair, naming the likely easting/northing swap.
fn check_utm_ranges(easting: f64, northing: f64, line_no: usize) -> Result<(), String> {
    if !(100_000.0..=900_000.0).contains(&easting) {
        let swapped = (100_000.0..=900_000.0).contains(&northing)
            && (0.0..=FALSE_NORTHING).contains(&easting);
        let hint = if swapped || easting > 1_000_000.0 {
            " — the easting and northing columns look swapped"
        } else {
            ""
        };
        return Err(format!(
            "row {line_no}: easting {easting} is outside the 100000 to 900000 metre range a UTM \
             zone can hold{hint}"
        ));
    }
    if !(0.0..=FALSE_NORTHING).contains(&northing) {
        return Err(format!(
            "row {line_no}: northing {northing} is outside the 0 to {FALSE_NORTHING:.0} metre \
             range a UTM zone can hold"
        ));
    }
    Ok(())
}

/// Flag a zone whose latitude-band letter disagrees with the latitude it
/// produced — usually a mistyped band, or northern data labelled with a
/// southern band (or the reverse).
fn check_band(spec: ZoneSpec, lat: f64, line_no: usize) -> Result<(), String> {
    let Some(letter) = spec.band else {
        return Ok(());
    };
    let Some((low, high)) = band_range(letter) else {
        return Ok(());
    };
    // UTM zones overlap slightly at their edges, so allow a degree of slack.
    if lat >= low - 1.0 && lat <= high + 1.0 {
        return Ok(());
    }
    Err(format!(
        "row {line_no}: zone {}{letter} is latitude band {letter} ({low:.0}° to {high:.0}°), but \
         these coordinates land at {lat:.4}° — check the band letter, or set hemisphere to north \
         or south to override it",
        spec.number
    ))
}

// ---------------------------------------------------------------------------
// CSV plumbing
// ---------------------------------------------------------------------------

/// Pick the field delimiter, sniffing the first line when `spec` is `auto`.
fn resolve_delimiter(spec: &str, csv_text: &str) -> Result<u8, String> {
    match spec.trim().to_ascii_lowercase().as_str() {
        "comma" => Ok(b','),
        "semicolon" => Ok(b';'),
        "tab" => Ok(b'\t'),
        "pipe" => Ok(b'|'),
        "" | "auto" => {
            let first = csv_text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
            Ok([b',', b';', b'\t', b'|']
                .into_iter()
                .map(|d| (first.bytes().filter(|b| *b == d).count(), d))
                .max_by_key(|(count, _)| *count)
                .map(|(count, d)| if count == 0 { b',' } else { d })
                .unwrap_or(b','))
        }
        other => Err(format!(
            "unknown delimiter \"{other}\" — use auto, comma, semicolon, tab or pipe"
        )),
    }
}

/// Parse the CSV text into records, skipping blank lines.
fn read_records(csv_text: &str, delimiter: u8) -> Result<Vec<Vec<String>>, String> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(csv_text.as_bytes());
    let mut out = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|e| format!("CSV parse error: {e}"))?;
        let fields: Vec<String> = record.iter().map(str::to_string).collect();
        if fields.iter().all(|f| f.is_empty()) {
            continue;
        }
        out.push(fields);
    }
    Ok(out)
}

/// Resolve a coordinate column from a header name, a 1-based index, or the aliases.
fn resolve_column(
    spec: &str,
    headers: &[String],
    aliases: &[&str],
    param: &str,
    label: &str,
    fallback: usize,
) -> Result<usize, String> {
    let spec = spec.trim();
    if !spec.is_empty() {
        return resolve_named(spec, headers, param);
    }
    if let Some(i) = headers
        .iter()
        .position(|h| aliases.iter().any(|a| h.trim().eq_ignore_ascii_case(a)))
    {
        return Ok(i);
    }
    if fallback < headers.len() {
        return Ok(fallback);
    }
    Err(format!(
        "could not find the {label} column — the CSV has {} column(s); set {param} explicitly",
        headers.len()
    ))
}

/// Resolve the optional zone column: explicit spec, else an auto-detected
/// `zone`-style header, else `None`.
fn resolve_zone_column(spec: &str, headers: &[String]) -> Result<Option<usize>, String> {
    let spec = spec.trim();
    if !spec.is_empty() {
        return resolve_named(spec, headers, "zone_column").map(Some);
    }
    Ok(headers
        .iter()
        .position(|h| ZONE_ALIASES.iter().any(|a| h.trim().eq_ignore_ascii_case(a))))
}

/// Resolve a user-supplied column spec: a 1-based index or a header name.
fn resolve_named(spec: &str, headers: &[String], param: &str) -> Result<usize, String> {
    if let Ok(n) = spec.parse::<usize>() {
        if n == 0 || n > headers.len() {
            return Err(format!(
                "{param} index {n} is out of range — the CSV has {} columns",
                headers.len()
            ));
        }
        return Ok(n - 1);
    }
    headers
        .iter()
        .position(|h| h.trim().eq_ignore_ascii_case(spec))
        .ok_or_else(|| {
            format!(
                "{param} \"{spec}\" is not a column — available columns: {}",
                headers.join(", ")
            )
        })
}

/// Fetch one cell, reporting a ragged row rather than silently padding it.
fn cell<'a>(
    row: &'a [String],
    idx: usize,
    line_no: usize,
    headers: &[String],
) -> Result<&'a str, String> {
    row.get(idx).map(String::as_str).ok_or_else(|| {
        format!(
            "row {line_no} has {} field(s) but column \"{}\" is #{}",
            row.len(),
            headers.get(idx).map(String::as_str).unwrap_or("?"),
            idx + 1
        )
    })
}

/// Parse one numeric cell, naming the row and column when it is not a number.
fn parse_number(raw: &str, line_no: usize, headers: &[String], idx: usize) -> Result<f64, String> {
    let cleaned = raw.trim();
    let name = headers.get(idx).map(String::as_str).unwrap_or("?");
    if cleaned.is_empty() {
        return Err(format!("row {line_no}: column \"{name}\" is empty"));
    }
    let value: f64 = cleaned.parse().map_err(|_| {
        format!("row {line_no}: column \"{name}\" value \"{cleaned}\" is not a number")
    })?;
    if !value.is_finite() {
        return Err(format!(
            "row {line_no}: column \"{name}\" value \"{cleaned}\" is not finite"
        ));
    }
    Ok(value)
}

/// Parse a latitude or longitude written as decimal degrees (`-33.8688`), as
/// degrees/minutes/seconds (`33°52'7.68"S`, `33 52 7.68 S`) or as
/// degrees/decimal-minutes (`33°52.128'S`).
fn parse_angle(
    raw: &str,
    line_no: usize,
    headers: &[String],
    idx: usize,
    is_latitude: bool,
) -> Result<f64, String> {
    let name = headers.get(idx).map(String::as_str).unwrap_or("?");
    let cleaned = raw.trim();
    let what = if is_latitude { "latitude" } else { "longitude" };
    if cleaned.is_empty() {
        return Err(format!("row {line_no}: column \"{name}\" is empty"));
    }
    let bad = || {
        format!("row {line_no}: column \"{name}\" value \"{cleaned}\" is not a {what} — write it as decimal degrees like -33.8688, or as 33°52'7.7\"S")
    };

    let mut parts: Vec<f64> = Vec::new();
    let mut hemi: Option<char> = None;
    let mut token = String::new();
    let flush = |token: &mut String, parts: &mut Vec<f64>| -> Result<(), ()> {
        if !token.is_empty() {
            parts.push(token.parse().map_err(|_| ())?);
            token.clear();
        }
        Ok(())
    };
    for ch in cleaned.chars() {
        match ch {
            '0'..='9' | '.' => token.push(ch),
            '-' | '+' if token.is_empty() => token.push(ch),
            'n' | 'N' | 's' | 'S' | 'e' | 'E' | 'w' | 'W' => {
                flush(&mut token, &mut parts).map_err(|()| bad())?;
                if hemi.is_some() {
                    return Err(bad());
                }
                hemi = Some(ch.to_ascii_uppercase());
            }
            _ => flush(&mut token, &mut parts).map_err(|()| bad())?,
        }
    }
    flush(&mut token, &mut parts).map_err(|()| bad())?;

    if parts.is_empty() || parts.len() > 3 || !parts.iter().all(|v| v.is_finite()) {
        return Err(bad());
    }
    if let Some(h) = hemi {
        let ok = if is_latitude {
            h == 'N' || h == 'S'
        } else {
            h == 'E' || h == 'W'
        };
        if !ok {
            return Err(format!(
                "row {line_no}: column \"{name}\" value \"{cleaned}\" ends in \"{h}\", which is \
                 not a {what} hemisphere"
            ));
        }
    }
    let negative = parts[0] < 0.0;
    let mut value = parts[0].abs();
    for (i, part) in parts.iter().enumerate().skip(1) {
        if !(0.0..60.0).contains(part) {
            return Err(bad());
        }
        value += part / if i == 1 { 60.0 } else { 3600.0 };
    }
    if negative || matches!(hemi, Some('S') | Some('W')) {
        value = -value;
    }

    let limit = if is_latitude { 90.0 } else { 360.0 };
    if value.abs() > limit {
        return Err(format!(
            "row {line_no}: column \"{name}\" value \"{cleaned}\" is not a {what} — it must be \
             within ±{limit:.0}°"
        ));
    }
    Ok(value)
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

/// Render a value at fixed precision, collapsing negative zero to `0`.
fn format_number(value: f64, decimals: usize) -> String {
    let rendered = format!("{value:.decimals$}");
    if rendered.starts_with('-') && rendered.bytes().all(|b| matches!(b, b'-' | b'0' | b'.')) {
        return rendered[1..].to_string();
    }
    rendered
}

fn format_latitude(lat: f64, opts: &Options) -> String {
    format_angle(lat, 'N', 'S', opts)
}

fn format_longitude(lon: f64, opts: &Options) -> String {
    format_angle(lon, 'E', 'W', opts)
}

/// Render one angle in the requested coordinate format. `decimals` counts
/// decimal-degree places; the sexagesimal forms drop 4 (seconds) or 2 (minutes)
/// of them, so every format carries roughly the same precision.
fn format_angle(value: f64, positive: char, negative: char, opts: &Options) -> String {
    match opts.coord_format {
        CoordFormat::Decimal => format_number(value, opts.decimals),
        CoordFormat::Dms => {
            let places = opts.decimals.saturating_sub(4);
            // Round to the requested second precision BEFORE splitting, so a
            // 59.999″ value carries into the next minute instead of printing 60″.
            let total = round_to(value.abs() * 3600.0, places);
            let deg = (total / 3600.0).floor();
            let rest = total - deg * 3600.0;
            let min = (rest / 60.0).floor();
            let sec = rest - min * 60.0;
            let width = if places > 0 { places + 3 } else { 2 };
            let hemi = if value < 0.0 { negative } else { positive };
            format!(
                "{deg:.0}°{min:02.0}'{sec:0width$.places$}\"{hemi}",
                width = width,
                places = places
            )
        }
        CoordFormat::Ddm => {
            let places = opts.decimals.saturating_sub(2);
            let total = round_to(value.abs() * 60.0, places);
            let deg = (total / 60.0).floor();
            let min = total - deg * 60.0;
            let width = if places > 0 { places + 3 } else { 2 };
            let hemi = if value < 0.0 { negative } else { positive };
            format!(
                "{deg:.0}°{min:0width$.places$}'{hemi}",
                width = width,
                places = places
            )
        }
    }
}

/// Round to `places` decimals, the way the formatter would.
fn round_to(value: f64, places: usize) -> f64 {
    let scale = 10f64.powi(places as i32);
    (value * scale).round() / scale
}

// ---------------------------------------------------------------------------
// Output shapes
// ---------------------------------------------------------------------------

/// Serialize the converted table in the requested output shape.
fn render(
    headers: &[String],
    rows: &[Vec<String>],
    points: &[(f64, f64)],
    opts: &Options,
) -> String {
    match opts.output {
        Output::Csv | Output::Tsv => {
            let delim = if opts.output == Output::Tsv {
                b'\t'
            } else {
                opts.delimiter
            };
            let mut writer = csv::WriterBuilder::new()
                .delimiter(delim)
                .from_writer(Vec::new());
            if opts.has_header {
                let _ = writer.write_record(headers);
            }
            for row in rows {
                let _ = writer.write_record(row);
            }
            let bytes = writer.into_inner().unwrap_or_default();
            String::from_utf8_lossy(&bytes).into_owned()
        }
        Output::Json => {
            let mut out = String::from("[\n");
            for (n, row) in rows.iter().enumerate() {
                out.push_str("  ");
                out.push_str(&json_object(headers, row));
                if n + 1 < rows.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str("]\n");
            out
        }
        Output::Table => {
            let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
            for row in rows {
                for (i, value) in row.iter().enumerate() {
                    if i < widths.len() {
                        widths[i] = widths[i].max(value.chars().count());
                    }
                }
            }
            let line = |cells: &[String]| -> String {
                cells
                    .iter()
                    .enumerate()
                    .map(|(i, c)| format!("{:>width$}", c, width = widths[i]))
                    .collect::<Vec<_>>()
                    .join("  ")
                    .trim_end()
                    .to_string()
            };
            let mut out = String::new();
            if opts.has_header {
                out.push_str(&line(headers));
                out.push('\n');
                out.push_str(
                    &widths
                        .iter()
                        .map(|w| "-".repeat(*w))
                        .collect::<Vec<_>>()
                        .join("  "),
                );
                out.push('\n');
            }
            for row in rows {
                out.push_str(&line(row));
                out.push('\n');
            }
            out
        }
        Output::GeoJson => {
            let mut out = String::from("{\n  \"type\": \"FeatureCollection\",\n  \"features\": [\n");
            for (n, row) in rows.iter().enumerate() {
                let (lon, lat) = points.get(n).copied().unwrap_or((0.0, 0.0));
                out.push_str("    { \"type\": \"Feature\", \"geometry\": { \"type\": \"Point\", \"coordinates\": [");
                out.push_str(&format_number(lon, opts.decimals));
                out.push_str(", ");
                out.push_str(&format_number(lat, opts.decimals));
                out.push_str("] }, \"properties\": ");
                out.push_str(&json_object(headers, row));
                out.push_str(" }");
                if n + 1 < rows.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str("  ]\n}\n");
            out
        }
        Output::Kml => {
            let mut out = String::from(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                 <kml xmlns=\"http://www.opengis.net/kml/2.2\">\n  <Document>\n",
            );
            // The first carried-through column makes the most useful label;
            // without one, the placemarks are simply numbered.
            let converted = match opts.direction {
                Direction::UtmToLatLon => 2,
                Direction::LatLonToUtm => 4,
            };
            let has_kept = headers.len() > converted;
            for (n, row) in rows.iter().enumerate() {
                let (lon, lat) = points.get(n).copied().unwrap_or((0.0, 0.0));
                let name = row
                    .first()
                    .filter(|_| has_kept)
                    .filter(|v| !v.trim().is_empty())
                    .cloned()
                    .unwrap_or_else(|| format!("Point {}", n + 1));
                out.push_str("    <Placemark>\n      <name>");
                out.push_str(&xml_escape(&name));
                out.push_str("</name>\n      <ExtendedData>\n");
                for (i, header) in headers.iter().enumerate() {
                    out.push_str("        <Data name=\"");
                    out.push_str(&xml_escape(header));
                    out.push_str("\"><value>");
                    out.push_str(&xml_escape(row.get(i).map(String::as_str).unwrap_or("")));
                    out.push_str("</value></Data>\n");
                }
                out.push_str("      </ExtendedData>\n      <Point><coordinates>");
                out.push_str(&format_number(lon, opts.decimals));
                out.push(',');
                out.push_str(&format_number(lat, opts.decimals));
                out.push_str("</coordinates></Point>\n    </Placemark>\n");
            }
            out.push_str("  </Document>\n</kml>\n");
            out
        }
    }
}

/// One row as a JSON object, keeping numeric-looking cells unquoted.
fn json_object(headers: &[String], row: &[String]) -> String {
    let mut out = String::from("{");
    for (i, header) in headers.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        let value = row.get(i).map(String::as_str).unwrap_or("");
        out.push_str(&json_string(header));
        out.push_str(": ");
        if value.parse::<f64>().map(f64::is_finite).unwrap_or(false) {
            out.push_str(value);
        } else {
            out.push_str(&json_string(value));
        }
    }
    out.push('}');
    out
}

/// Minimal JSON string escaping for header names and passthrough cells.
fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Escape the five XML metacharacters for KML text nodes and attributes.
fn xml_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UTM → lat/lon with the defaults the page and CLI ship.
    fn to_latlon(csv: &str) -> Result<String, String> {
        convert(
            csv, "utm_to_latlon", "", "", "", "", "auto", "decimal", 6, "wgs84", "auto", true,
            true, true, "csv",
        )
    }

    #[test]
    fn converts_a_headered_csv_to_decimal_degrees() {
        let out = to_latlon("id,easting,northing,zone\np1,583960,4507523,18T\n").unwrap();
        assert_eq!(out, "id,latitude,longitude\np1,40.714349,-74.005970\n");
    }

    #[test]
    fn southern_hemisphere_comes_from_the_band_letter() {
        let out = to_latlon("easting,northing,zone\n334519,6251430,56H\n").unwrap();
        assert_eq!(out, "latitude,longitude\n-33.864482,151.211016\n");
    }

    /// Snyder, USGS Professional Paper 1395, Transverse Mercator worked example:
    /// Clarke 1866, k0 = 0.9996, central meridian 75°W, φ = 40°30′N, λ = 73°30′W
    /// gives x = 127 106.5 m from the central meridian and y = 4 484 124.4 m.
    #[test]
    fn matches_the_published_snyder_reference_point() {
        let tm = Tm::new(6_378_206.4, 294.978_698_2);
        let (easting, northing) = tm.from_latlon(40.5, -73.5, 18);
        assert!(
            (easting - FALSE_EASTING - 127_106.5).abs() < 0.1,
            "easting offset {}",
            easting - FALSE_EASTING
        );
        assert!(
            (northing - 4_484_124.4).abs() < 0.1,
            "northing {northing}"
        );
        let (lat, lon) = tm.to_latlon(easting, northing, 18, true);
        assert!((lat - 40.5).abs() < 1e-9 && (lon + 73.5).abs() < 1e-9, "{lat} {lon}");
    }

    #[test]
    fn round_trips_every_zone_in_both_hemispheres_to_under_a_millimetre() {
        let tm = Tm::new(6_378_137.0, 298.257_223_563);
        let mut worst: f64 = 0.0;
        for zone in 1..=60u8 {
            for lat in [-79.5, -33.8688, -0.001, 0.001, 40.7128, 60.0, 83.9] {
                for offset in [-2.9, 0.0, 2.9] {
                    let lon = wrap_pi((central_meridian(zone) + offset).to_radians()).to_degrees();
                    let (easting, northing) = tm.from_latlon(lat, lon, zone);
                    let (back_lat, back_lon) = tm.to_latlon(easting, northing, zone, lat >= 0.0);
                    worst = worst.max((back_lat - lat).abs().max((back_lon - lon).abs()));
                }
            }
        }
        // 1e-8° of latitude is about 1.1 mm.
        assert!(worst < 1e-8, "worst round-trip error {worst}°");
    }

    #[test]
    fn a_single_zone_can_cover_a_whole_headerless_file() {
        let out = convert(
            "583960,4507523\n584000,4507600\n",
            "utm_to_latlon",
            "",
            "",
            "",
            "18N",
            "auto",
            "decimal",
            4,
            "wgs84",
            "auto",
            false,
            false,
            true,
            "csv",
        )
        .unwrap();
        assert_eq!(out, "40.7143,-74.0060\n40.7150,-74.0055\n");
    }

    #[test]
    fn every_zone_spelling_resolves_to_the_same_point() {
        let expected = "latitude,longitude\n40.714349,-74.005970\n";
        for spelling in ["18", "18T", "18 T", "18N", "zone 18", "EPSG:32618", "32618"] {
            let out = convert(
                "easting,northing\n583960,4507523\n",
                "utm_to_latlon",
                "",
                "",
                "",
                spelling,
                "auto",
                "decimal",
                6,
                "wgs84",
                "auto",
                true,
                true,
                true,
                "csv",
            )
            .unwrap();
            assert_eq!(out, expected, "zone spelling {spelling}");
        }
        // "56S" follows the GIS naming convention (zone 56, southern
        // hemisphere), and agrees with the EPSG code and the band letter.
        for spelling in ["56S", "EPSG:32756", "32756", "56H"] {
            let south = convert(
                "easting,northing\n334519,6251430\n",
                "utm_to_latlon",
                "",
                "",
                "",
                spelling,
                "auto",
                "decimal",
                6,
                "wgs84",
                "auto",
                true,
                true,
                true,
                "csv",
            )
            .unwrap();
            assert_eq!(
                south, "latitude,longitude\n-33.864482,151.211016\n",
                "zone spelling {spelling}"
            );
        }
    }

    #[test]
    fn dms_and_ddm_formats_carry_the_same_precision() {
        let dms = convert(
            "easting,northing,zone\n583960,4507523,18T\n",
            "utm_to_latlon", "", "", "", "", "auto", "dms", 6, "wgs84", "auto", true, true, true,
            "csv",
        )
        .unwrap();
        assert_eq!(
            dms,
            "latitude,longitude\n\"40°42'51.66\"\"N\",\"74°00'21.49\"\"W\"\n"
        );

        let ddm = convert(
            "easting,northing,zone\n583960,4507523,18T\n",
            "utm_to_latlon", "", "", "", "", "auto", "ddm", 6, "wgs84", "auto", true, true, true,
            "csv",
        )
        .unwrap();
        assert_eq!(ddm, "latitude,longitude\n40°42.8610'N,74°00.3582'W\n");
    }

    #[test]
    fn each_ellipsoid_moves_the_answer_by_its_own_amount() {
        let mut seen = Vec::new();
        for ellipsoid in ["wgs84", "grs80", "clarke1866", "international1924"] {
            let out = convert(
                "easting,northing,zone\n583960,4507523,18T\n",
                "utm_to_latlon", "", "", "", "", "auto", "decimal", 6, ellipsoid, "auto", true,
                true, true, "csv",
            )
            .unwrap();
            seen.push(out);
        }
        assert_eq!(seen[0], "latitude,longitude\n40.714349,-74.005970\n");
        // GRS80 and WGS84 differ only in the 9th decimal of the flattening.
        assert_eq!(seen[0], seen[1]);
        assert_eq!(seen[2], "latitude,longitude\n40.716253,-74.005968\n");
        assert_eq!(seen[3], "latitude,longitude\n40.713645,-74.006025\n");
    }

    #[test]
    fn reverse_direction_picks_the_zone_and_band_from_the_position() {
        let out = convert(
            "id,latitude,longitude\nnyc,40.7128,-74.0060\nsyd,-33.8688,151.2093\n",
            "latlon_to_utm", "", "", "", "", "auto", "decimal", 3, "wgs84", "auto", true, true,
            true, "csv",
        )
        .unwrap();
        assert_eq!(
            out,
            "id,easting,northing,zone,hemisphere\n\
             nyc,583959.372,4507350.998,18T,N\n\
             syd,334368.634,6250948.345,56H,S\n"
        );
    }

    #[test]
    fn reverse_direction_applies_the_norway_and_svalbard_zone_exceptions() {
        let out = convert(
            "latitude,longitude\n59,5\n78,15\n78,25\n78,35\n",
            "latlon_to_utm", "", "", "", "", "auto", "decimal", 0, "wgs84", "auto", true, false,
            true, "csv",
        )
        .unwrap();
        assert_eq!(
            out,
            "easting,northing,zone,hemisphere\n\
             270278,6546930,32V,N\n\
             500000,8658370,33X,N\n\
             453589,8659162,35X,N\n\
             407230,8661539,37X,N\n"
        );
    }

    #[test]
    fn reverse_direction_reads_degrees_minutes_seconds() {
        let out = convert(
            "latitude,longitude\n40°42'46.08\"N,74°00'21.6\"W\n",
            "latlon_to_utm", "", "", "", "", "auto", "decimal", 3, "wgs84", "auto", true, false,
            true, "csv",
        )
        .unwrap();
        assert_eq!(out, "easting,northing,zone,hemisphere\n583959.372,4507350.998,18T,N\n");
    }

    #[test]
    fn geojson_and_kml_carry_decimal_geometry() {
        let geojson = convert(
            "id,easting,northing,zone\np1,583960,4507523,18T\n",
            "utm_to_latlon", "", "", "", "", "auto", "decimal", 6, "wgs84", "auto", true, true,
            true, "geojson",
        )
        .unwrap();
        assert_eq!(
            geojson,
            "{\n  \"type\": \"FeatureCollection\",\n  \"features\": [\n    \
             { \"type\": \"Feature\", \"geometry\": { \"type\": \"Point\", \"coordinates\": \
             [-74.005970, 40.714349] }, \"properties\": {\"id\": \"p1\", \"latitude\": 40.714349, \
             \"longitude\": -74.005970} }\n  ]\n}\n"
        );

        let kml = convert(
            "id,easting,northing,zone\np1,583960,4507523,18T\n",
            "utm_to_latlon", "", "", "", "", "auto", "decimal", 6, "wgs84", "auto", true, true,
            true, "kml",
        )
        .unwrap();
        assert!(kml.contains("<coordinates>-74.005970,40.714349</coordinates>"), "{kml}");
        assert!(kml.contains("<name>p1</name>"), "{kml}");
    }

    #[test]
    fn tsv_json_and_table_outputs_render() {
        let tsv = convert(
            "easting,northing,zone\n583960,4507523,18T\n",
            "utm_to_latlon", "", "", "", "", "auto", "decimal", 4, "wgs84", "auto", true, true,
            true, "tsv",
        )
        .unwrap();
        assert_eq!(tsv, "latitude\tlongitude\n40.7143\t-74.0060\n");

        let json = convert(
            "id,easting,northing,zone\np1,583960,4507523,18T\n",
            "utm_to_latlon", "", "", "", "", "auto", "decimal", 4, "wgs84", "auto", true, true,
            true, "json",
        )
        .unwrap();
        assert_eq!(
            json,
            "[\n  {\"id\": \"p1\", \"latitude\": 40.7143, \"longitude\": -74.0060}\n]\n"
        );

        let table = convert(
            "easting,northing,zone\n583960,4507523,18T\n",
            "utm_to_latlon", "", "", "", "", "auto", "decimal", 4, "wgs84", "auto", true, true,
            true, "table",
        )
        .unwrap();
        assert_eq!(
            table,
            "latitude  longitude\n--------  ---------\n 40.7143   -74.0060\n"
        );
    }

    #[test]
    fn semicolon_delimiter_is_sniffed_and_echoed() {
        let out = to_latlon("easting;northing;zone\n583960;4507523;18T\n").unwrap();
        assert_eq!(out, "latitude;longitude\n40.714349;-74.005970\n");
    }

    #[test]
    fn columns_can_be_selected_by_name_or_index() {
        let by_name = convert(
            "label,e_m,n_m,z\nsite,583960,4507523,18T\n",
            "utm_to_latlon", "e_m", "n_m", "z", "", "auto", "decimal", 4, "wgs84", "auto", true,
            true, true, "csv",
        )
        .unwrap();
        assert_eq!(by_name, "label,latitude,longitude\nsite,40.7143,-74.0060\n");

        let by_index = convert(
            "a,b,c\n583960,4507523,18T\n",
            "utm_to_latlon", "1", "2", "3", "", "auto", "decimal", 4, "wgs84", "auto", true, false,
            true, "csv",
        )
        .unwrap();
        assert_eq!(by_index, "latitude,longitude\n40.7143,-74.0060\n");
    }

    #[test]
    fn a_band_letter_that_disagrees_with_the_result_is_reported() {
        // Band T covers 40°N-48°N, but a 6 251 430 m northing read as northern
        // lands near 56°N — the band letter and the coordinates disagree.
        let err = to_latlon("easting,northing,zone\n334519,6251430,18T\n").unwrap_err();
        assert!(err.contains("latitude band T"), "{err}");
        assert!(err.contains("set hemisphere to north"), "{err}");

        // Forcing the hemisphere is the documented escape hatch: it silences
        // the band cross-check and projects exactly what was asked for.
        let forced = convert(
            "easting,northing,zone\n334519,6251430,18T\n",
            "utm_to_latlon", "", "", "", "", "north", "decimal", 4, "wgs84", "auto", true, true,
            true, "csv",
        )
        .unwrap();
        assert!(forced.starts_with("latitude,longitude\n56."), "{forced}");
    }

    #[test]
    fn swapped_easting_and_northing_are_named_as_such() {
        let err = to_latlon("easting,northing,zone\n4507523,583960,18T\n").unwrap_err();
        assert!(err.contains("row 2"), "{err}");
        assert!(err.contains("look swapped"), "{err}");

        // With validation off the tool projects whatever it is given.
        let out = convert(
            "easting,northing,zone\n4507523,583960,18T\n",
            "utm_to_latlon", "", "", "", "", "north", "decimal", 2, "wgs84", "auto", true, true,
            false, "csv",
        )
        .unwrap();
        assert!(out.starts_with("latitude,longitude\n"), "{out}");
    }

    #[test]
    fn a_missing_zone_is_reported_before_any_row_is_read() {
        let err = to_latlon("easting,northing\n583960,4507523\n").unwrap_err();
        assert!(err.contains("no UTM zone found"), "{err}");
    }

    #[test]
    fn latitudes_beyond_utm_are_rejected_rather_than_extrapolated() {
        let err = convert(
            "latitude,longitude\n86.4,10\n",
            "latlon_to_utm", "", "", "", "", "auto", "decimal", 3, "wgs84", "auto", true, false,
            true, "csv",
        )
        .unwrap_err();
        assert!(err.contains("UPS grid"), "{err}");
    }

    #[test]
    fn bad_cells_and_bad_options_name_what_is_wrong() {
        let err = to_latlon("easting,northing,zone\nnope,4507523,18T\n").unwrap_err();
        assert_eq!(
            err,
            "row 2: column \"easting\" value \"nope\" is not a number"
        );

        let err = to_latlon("easting,northing,zone\n583960,4507523,99\n").unwrap_err();
        assert!(err.contains("UTM zones run 1 to 60"), "{err}");

        let err = to_latlon("easting,northing,zone\n583960,4507523,18I\n").unwrap_err();
        assert!(err.contains("not a UTM latitude band"), "{err}");

        let err = convert(
            "easting,northing,zone\n583960,4507523,18T\n",
            "utm_to_latlon", "", "", "", "", "auto", "decimal", 99, "wgs84", "auto", true, true,
            true, "csv",
        )
        .unwrap_err();
        assert!(err.contains("decimals must be between 0 and 12"), "{err}");

        let err = convert(
            "easting,northing,zone\n583960,4507523,18T\n",
            "utm_to_latlon", "", "", "", "", "auto", "decimal", 6, "bessel", "auto", true, true,
            true, "csv",
        )
        .unwrap_err();
        assert!(err.contains("unknown ellipsoid"), "{err}");

        assert!(to_latlon("   ").unwrap_err().contains("empty"));
        assert!(to_latlon("easting,northing,zone\n").unwrap_err().contains("no data rows"));
    }
}
