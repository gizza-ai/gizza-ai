//! unit-converter core — pure compute, shared by the chat skill block and the web page.
//! Converts a numeric value between two units of the same physical category
//! (length, mass, temperature, volume, area, speed, data size, time). Each
//! linear category maps every unit to a base-unit factor and converts via the
//! base; temperature is affine, so it has its own to/from-base helpers.
//! No wafer/wasm-bindgen deps.

/// A unit: its canonical name plus accepted aliases, and how it relates to the
/// category base unit. For linear categories `factor` = "how many base units in
/// one of this unit" (e.g. km -> 1000 metres). Temperature uses the `to_base` /
/// `from_base` closures-free path in `convert`.
struct Unit {
    canonical: &'static str,
    aliases: &'static [&'static str],
    /// base units per one of this unit (linear categories only).
    factor: f64,
}

#[allow(dead_code)]
struct Category {
    name: &'static str,
    base: &'static str,
    units: &'static [Unit],
}

macro_rules! unit {
    ($canon:expr, [$($a:expr),*], $f:expr) => {
        Unit { canonical: $canon, aliases: &[$($a),*], factor: $f }
    };
}

// ---- Length (base: metre) ----
const LENGTH: &[Unit] = &[
    unit!("metre", ["meter", "metres", "meters", "m"], 1.0),
    unit!("kilometre", ["kilometer", "km", "kilometres", "kilometers"], 1000.0),
    unit!("centimetre", ["centimeter", "cm", "centimetres", "centimeters"], 0.01),
    unit!("millimetre", ["millimeter", "mm", "millimetres", "millimeters"], 0.001),
    unit!("micrometre", ["micrometer", "micron", "um", "µm"], 1e-6),
    unit!("nanometre", ["nanometer", "nm"], 1e-9),
    unit!("mile", ["miles", "mi"], 1609.344),
    unit!("yard", ["yards", "yd"], 0.9144),
    unit!("foot", ["feet", "ft"], 0.3048),
    unit!("inch", ["inches", "in"], 0.0254),
    unit!("nautical-mile", ["nautical_mile", "nmi", "nautical-miles"], 1852.0),
];

// ---- Mass (base: kilogram) ----
const MASS: &[Unit] = &[
    unit!("kilogram", ["kilograms", "kg"], 1.0),
    unit!("gram", ["grams", "g"], 0.001),
    unit!("milligram", ["milligrams", "mg"], 1e-6),
    unit!("microgram", ["micrograms", "ug", "µg"], 1e-9),
    unit!("tonne", ["tonnes", "metric-ton", "t"], 1000.0),
    unit!("pound", ["pounds", "lb", "lbs"], 0.45359237),
    unit!("ounce", ["ounces", "oz"], 0.028349523125),
    unit!("stone", ["stones", "st"], 6.35029318),
    unit!("ton-us", ["short-ton", "us-ton"], 907.18474),
    unit!("ton-uk", ["long-ton", "imperial-ton"], 1016.0469088),
];

// ---- Volume (base: litre) ----
const VOLUME: &[Unit] = &[
    unit!("litre", ["liter", "litres", "liters", "l"], 1.0),
    unit!("millilitre", ["milliliter", "ml", "millilitres", "milliliters"], 0.001),
    unit!("cubic-metre", ["cubic-meter", "m3", "m^3"], 1000.0),
    unit!("cubic-centimetre", ["cubic-centimeter", "cc", "cm3"], 0.001),
    unit!("gallon-us", ["us-gallon", "gallon", "gal"], 3.785411784),
    unit!("gallon-uk", ["uk-gallon", "imperial-gallon"], 4.54609),
    unit!("quart-us", ["us-quart", "quart", "qt"], 0.946352946),
    unit!("pint-us", ["us-pint", "pint", "pt"], 0.473176473),
    unit!("cup-us", ["us-cup", "cup"], 0.2365882365),
    unit!("fluid-ounce-us", ["us-fluid-ounce", "fluid-ounce", "fl-oz", "floz"], 0.0295735295625),
    unit!("tablespoon", ["tablespoons", "tbsp"], 0.01478676478125),
    unit!("teaspoon", ["teaspoons", "tsp"], 0.00492892159375),
];

// ---- Area (base: square metre) ----
const AREA: &[Unit] = &[
    unit!("square-metre", ["square-meter", "sqm", "m2", "m^2"], 1.0),
    unit!("square-kilometre", ["square-kilometer", "sqkm", "km2", "km^2"], 1e6),
    unit!("square-centimetre", ["square-centimeter", "cm2", "cm^2"], 1e-4),
    unit!("square-millimetre", ["square-millimeter", "mm2", "mm^2"], 1e-6),
    unit!("hectare", ["hectares", "ha"], 10000.0),
    unit!("acre", ["acres", "ac"], 4046.8564224),
    unit!("square-mile", ["sqmi", "mi2", "mi^2"], 2589988.110336),
    unit!("square-yard", ["sqyd", "yd2", "yd^2"], 0.83612736),
    unit!("square-foot", ["square-feet", "sqft", "ft2", "ft^2"], 0.09290304),
    unit!("square-inch", ["sqin", "in2", "in^2"], 0.00064516),
];

// ---- Speed (base: metre per second) ----
const SPEED: &[Unit] = &[
    unit!("metre-per-second", ["meter-per-second", "mps", "m/s"], 1.0),
    unit!("kilometre-per-hour", ["kilometer-per-hour", "kmh", "kph", "km/h"], 0.2777777777777778),
    unit!("mile-per-hour", ["miles-per-hour", "mph", "mi/h"], 0.44704),
    unit!("foot-per-second", ["feet-per-second", "fps", "ft/s"], 0.3048),
    unit!("knot", ["knots", "kn", "kt"], 0.5144444444444445),
];

// ---- Data size (base: byte; decimal SI + binary IEC) ----
const DATA: &[Unit] = &[
    unit!("byte", ["bytes", "b"], 1.0),
    unit!("bit", ["bits"], 0.125),
    unit!("kilobyte", ["kilobytes", "kb"], 1e3),
    unit!("megabyte", ["megabytes", "mb"], 1e6),
    unit!("gigabyte", ["gigabytes", "gb"], 1e9),
    unit!("terabyte", ["terabytes", "tb"], 1e12),
    unit!("petabyte", ["petabytes", "pb"], 1e15),
    unit!("kibibyte", ["kibibytes", "kib"], 1024.0),
    unit!("mebibyte", ["mebibytes", "mib"], 1048576.0),
    unit!("gibibyte", ["gibibytes", "gib"], 1073741824.0),
    unit!("tebibyte", ["tebibytes", "tib"], 1099511627776.0),
];

// ---- Time (base: second) ----
const TIME: &[Unit] = &[
    unit!("second", ["seconds", "s", "sec", "secs"], 1.0),
    unit!("millisecond", ["milliseconds", "ms"], 0.001),
    unit!("microsecond", ["microseconds", "us", "µs"], 1e-6),
    unit!("nanosecond", ["nanoseconds", "ns"], 1e-9),
    unit!("minute", ["minutes", "min", "mins"], 60.0),
    unit!("hour", ["hours", "h", "hr", "hrs"], 3600.0),
    unit!("day", ["days", "d"], 86400.0),
    unit!("week", ["weeks", "wk", "wks"], 604800.0),
    unit!("month", ["months", "mo"], 2629800.0), // average Gregorian month
    unit!("year", ["years", "yr", "yrs"], 31557600.0), // Julian year (365.25 d)
];

// Temperature is affine (offset + scale), handled specially.
const TEMP_UNITS: &[(&str, &[&str])] = &[
    ("celsius", &["c", "centigrade", "°c", "degc"]),
    ("fahrenheit", &["f", "°f", "degf"]),
    ("kelvin", &["k", "°k"]),
    ("rankine", &["r", "°r", "ra"]),
    ("reaumur", &["re", "°re", "réaumur", "reaumur-scale"]),
];

const CATEGORIES: &[Category] = &[
    Category { name: "length", base: "metre", units: LENGTH },
    Category { name: "mass", base: "kilogram", units: MASS },
    Category { name: "volume", base: "litre", units: VOLUME },
    Category { name: "area", base: "square-metre", units: AREA },
    Category { name: "speed", base: "metre-per-second", units: SPEED },
    Category { name: "data", base: "byte", units: DATA },
    Category { name: "time", base: "second", units: TIME },
];

fn norm(s: &str) -> String {
    s.trim().to_ascii_lowercase()
}

/// Find the unit matching `name` (by canonical or alias) within a category.
fn find_unit<'a>(cat: &'a Category, name: &str) -> Option<&'a Unit> {
    let n = norm(name);
    cat.units.iter().find(|u| {
        u.canonical == n || u.aliases.iter().any(|a| *a == n)
    })
}

/// Resolve a temperature unit to its canonical name, or None.
fn find_temp(name: &str) -> Option<&'static str> {
    let n = norm(name);
    TEMP_UNITS
        .iter()
        .find(|(canon, aliases)| *canon == n || aliases.iter().any(|a| *a == n))
        .map(|(c, _)| *c)
}

fn temp_to_celsius(value: f64, unit: &str) -> f64 {
    match unit {
        "celsius" => value,
        "fahrenheit" => (value - 32.0) * 5.0 / 9.0,
        "kelvin" => value - 273.15,
        "rankine" => (value - 491.67) * 5.0 / 9.0,
        "reaumur" => value * 5.0 / 4.0,
        _ => value,
    }
}

fn temp_from_celsius(c: f64, unit: &str) -> f64 {
    match unit {
        "celsius" => c,
        "fahrenheit" => c * 9.0 / 5.0 + 32.0,
        "kelvin" => c + 273.15,
        "rankine" => (c + 273.15) * 9.0 / 5.0,
        "reaumur" => c * 4.0 / 5.0,
        _ => c,
    }
}

/// The result of a conversion.
#[derive(Debug, Clone)]
pub struct Conversion {
    pub value: f64,
    pub category: &'static str,
    pub from_unit: String,
    pub to_unit: String,
}

/// Convert `value` from unit `from` to unit `to`. The category is inferred from
/// the units; both units must belong to the same category (or both be
/// temperatures). Returns the converted value plus metadata.
pub fn convert(value: f64, from: &str, to: &str) -> Result<Conversion, String> {
    if !value.is_finite() {
        return Err("value must be a finite number".into());
    }
    let from_n = norm(from);
    let to_n = norm(to);
    if from_n.is_empty() || to_n.is_empty() {
        return Err("both 'from' and 'to' units are required".into());
    }

    // Temperature path.
    if let (Some(fu), Some(tu)) = (find_temp(&from_n), find_temp(&to_n)) {
        let c = temp_to_celsius(value, fu);
        let out = temp_from_celsius(c, tu);
        return Ok(Conversion {
            value: out,
            category: "temperature",
            from_unit: fu.to_string(),
            to_unit: tu.to_string(),
        });
    }

    // Linear categories. Find the category that contains `from`.
    let mut from_cat: Option<&Category> = None;
    let mut from_unit: Option<&Unit> = None;
    for cat in CATEGORIES {
        if let Some(u) = find_unit(cat, &from_n) {
            from_cat = Some(cat);
            from_unit = Some(u);
            break;
        }
    }
    let cat = from_cat.ok_or_else(|| format!("unknown unit '{from}'"))?;
    let fu = from_unit.unwrap();

    let tu = find_unit(cat, &to_n).ok_or_else(|| {
        // Help the user: is `to` valid but in a different category?
        if find_temp(&to_n).is_some() {
            return format!(
                "cannot convert {} ({}) to a temperature unit",
                fu.canonical, cat.name
            );
        }
        for other in CATEGORIES {
            if other.name != cat.name {
                if let Some(ou) = find_unit(other, &to_n) {
                    return format!(
                        "cannot convert {} ({}) to {} ({})",
                        fu.canonical, cat.name, ou.canonical, other.name
                    );
                }
            }
        }
        format!("unknown unit '{to}'")
    })?;

    let base = value * fu.factor;
    let out = base / tu.factor;
    Ok(Conversion {
        value: out,
        category: cat.name,
        from_unit: fu.canonical.to_string(),
        to_unit: tu.canonical.to_string(),
    })
}

/// Format a float for display: trims trailing zeros, avoids scientific notation
/// for reasonable magnitudes, keeps up to 12 significant digits.
pub fn fmt_value(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let abs = v.abs();
    // Use scientific notation for extreme magnitudes.
    if abs != 0.0 && (abs >= 1e15 || abs < 1e-9) {
        let s = format!("{:e}", v);
        return s;
    }
    // Round to 12 significant figures, then trim.
    let mut s = format!("{:.12}", v);
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    s
}

/// Convenience: convert and render a one-line "X unit = Y unit" string.
pub fn convert_to_string(value: f64, from: &str, to: &str) -> Result<String, String> {
    let c = convert(value, from, to)?;
    Ok(format!(
        "{} {} = {} {}",
        fmt_value(value),
        c.from_unit,
        fmt_value(c.value),
        c.to_unit
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "expected {b}, got {a}");
    }

    #[test]
    fn length_km_to_miles() {
        let c = convert(5.0, "km", "miles").unwrap();
        assert_eq!(c.category, "length");
        approx(c.value, 3.106855961);
    }

    #[test]
    fn length_inch_to_cm() {
        let c = convert(1.0, "inch", "cm").unwrap();
        approx(c.value, 2.54);
    }

    #[test]
    fn mass_pound_to_kg() {
        let c = convert(10.0, "lb", "kg").unwrap();
        approx(c.value, 4.5359237);
    }

    #[test]
    fn temp_c_to_f() {
        let c = convert(100.0, "celsius", "fahrenheit").unwrap();
        assert_eq!(c.category, "temperature");
        approx(c.value, 212.0);
    }

    #[test]
    fn temp_f_to_c() {
        approx(convert(32.0, "f", "c").unwrap().value, 0.0);
    }

    #[test]
    fn temp_c_to_k() {
        approx(convert(0.0, "c", "k").unwrap().value, 273.15);
    }

    #[test]
    fn temp_k_to_r() {
        approx(convert(0.0, "kelvin", "rankine").unwrap().value, 0.0);
        approx(convert(273.15, "kelvin", "rankine").unwrap().value, 491.67);
    }

    #[test]
    fn temp_c_to_reaumur() {
        approx(convert(100.0, "celsius", "reaumur").unwrap().value, 80.0);
        approx(convert(80.0, "reaumur", "celsius").unwrap().value, 100.0);
    }

    #[test]
    fn volume_gallon_to_litre() {
        approx(convert(1.0, "gallon", "litre").unwrap().value, 3.785411784);
    }

    #[test]
    fn area_acre_to_sqm() {
        approx(convert(1.0, "acre", "square-metre").unwrap().value, 4046.8564224);
    }

    #[test]
    fn speed_mph_to_kmh() {
        approx(convert(60.0, "mph", "kmh").unwrap().value, 96.56064);
    }

    #[test]
    fn data_gb_to_mb() {
        approx(convert(1.0, "gb", "mb").unwrap().value, 1000.0);
    }

    #[test]
    fn data_gib_to_mib() {
        approx(convert(1.0, "gib", "mib").unwrap().value, 1024.0);
    }

    #[test]
    fn time_hour_to_minutes() {
        approx(convert(2.0, "hours", "minutes").unwrap().value, 120.0);
    }

    #[test]
    fn round_trip() {
        let c = convert(123.456, "metre", "foot").unwrap();
        let back = convert(c.value, "foot", "metre").unwrap();
        approx(back.value, 123.456);
    }

    #[test]
    fn to_string_form() {
        let s = convert_to_string(1.0, "km", "m").unwrap();
        assert_eq!(s, "1 kilometre = 1000 metre");
    }

    #[test]
    fn unknown_unit_errors() {
        assert!(convert(1.0, "furlong", "metre").is_err());
        assert!(convert(1.0, "metre", "parsec").is_err());
    }

    #[test]
    fn cross_category_errors() {
        let e = convert(1.0, "metre", "kilogram").unwrap_err();
        assert!(e.contains("length") && e.contains("mass"), "got: {e}");
    }

    #[test]
    fn temp_to_length_errors() {
        assert!(convert(1.0, "celsius", "metre").is_err());
        assert!(convert(1.0, "metre", "celsius").is_err());
    }

    #[test]
    fn nonfinite_errors() {
        assert!(convert(f64::NAN, "m", "m").is_err());
        assert!(convert(f64::INFINITY, "m", "m").is_err());
    }

    #[test]
    fn fmt_trims_zeros() {
        assert_eq!(fmt_value(1.5), "1.5");
        assert_eq!(fmt_value(1000.0), "1000");
        assert_eq!(fmt_value(0.0), "0");
    }
}
