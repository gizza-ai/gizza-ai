//! form-field-validator core — validate a whole form submission field by field
//! against country (locale) rules and report a per-field verdict.
//!
//! Pure compute, shared by the chat skill block and the web page. No
//! wafer/wasm-bindgen deps, no I/O: every check is a format/locale check, never
//! a network lookup, so "well-formed" never means "exists" or "deliverable".

use serde_json::{json, Map, Value};

/// Hard cap on how many fields one submission may carry.
pub const MAX_FIELDS: usize = 200;

/// Enum vocabulary for the `country` param — `any` plus every code in
/// [`COUNTRIES`], in the same order. Kept as a fixed-size array because
/// `Param::enumv` takes `[&str; N]`.
pub const COUNTRY_CODES: [&str; 39] = [
    "any", "AR", "AT", "AU", "BE", "BR", "CA", "CH", "CN", "CZ", "DE", "DK", "ES", "FI", "FR",
    "GB", "GR", "HU", "IE", "IL", "IN", "IT", "JP", "KR", "MX", "NL", "NO", "NZ", "PL", "PT", "RO",
    "RU", "SE", "SG", "SK", "TR", "UA", "US", "ZA",
];

/// The field types this tool knows how to check.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Email,
    Phone,
    Url,
    PostalCode,
    CreditCard,
    Text,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Email => "email",
            Kind::Phone => "phone",
            Kind::Url => "url",
            Kind::PostalCode => "postal-code",
            Kind::CreditCard => "credit-card",
            Kind::Text => "text",
        }
    }

    /// Parse a type written in the `rules` param. Common spellings are accepted
    /// so `zip`, `postcode` and `postal_code` all mean the same thing.
    pub fn parse(s: &str) -> Option<Kind> {
        let k = s.trim().to_ascii_lowercase().replace('_', "-");
        match k.as_str() {
            "email" | "e-mail" | "mail" => Some(Kind::Email),
            "phone" | "tel" | "telephone" | "mobile" | "msisdn" => Some(Kind::Phone),
            "url" | "uri" | "link" | "website" => Some(Kind::Url),
            "postal-code" | "postal" | "postcode" | "post-code" | "zip" | "zipcode"
            | "zip-code" | "pincode" => Some(Kind::PostalCode),
            "credit-card" | "creditcard" | "card" | "cc" | "pan" => Some(Kind::CreditCard),
            "text" | "string" | "any" | "none" => Some(Kind::Text),
            _ => None,
        }
    }

    /// Guess the type from the field's name, the way a form builder binds a
    /// validator to a named input.
    pub fn infer(name: &str) -> Kind {
        let n = name.to_ascii_lowercase().replace(['_', '-', '.', ' '], "");
        let has = |needle: &str| n.contains(needle);
        if has("email") || has("mail") {
            return Kind::Email;
        }
        if has("phone") || has("mobile") || has("telephone") || has("tel") || has("fax") {
            return Kind::Phone;
        }
        if has("zip") || has("postal") || has("postcode") || has("pincode") {
            return Kind::PostalCode;
        }
        if has("card") || has("pan") || has("ccnumber") {
            return Kind::CreditCard;
        }
        if has("url") || has("website") || has("homepage") || has("link") || has("site") {
            return Kind::Url;
        }
        Kind::Text
    }
}

/// A country's phone + postal-code rules. `trunk` is the national dialling
/// prefix that replaces the calling code inside the country.
pub struct Country {
    pub code: &'static str,
    pub name: &'static str,
    /// Calling code without the leading `+`.
    pub calling: &'static str,
    pub trunk: Option<&'static str>,
    /// Inclusive length bounds of the national significant number.
    pub nsn_min: usize,
    pub nsn_max: usize,
    /// A well-known national number, used in the "Example:" hint.
    pub phone_example: &'static str,
    /// Accepted postal-code shapes: `N` = digit, `A` = letter,
    /// `C` = letter or digit; every other character is a literal separator.
    pub postal: &'static [&'static str],
    pub postal_example: &'static str,
}

const GB_POSTAL: &[&str] = &[
    "AN NAA", "ANN NAA", "AAN NAA", "AANN NAA", "ANA NAA", "AANA NAA",
];

/// Sorted by ISO 3166-1 alpha-2 code — the same order as [`COUNTRY_CODES`].
pub const COUNTRIES: &[Country] = &[
    Country {
        code: "AR",
        name: "Argentina",
        calling: "54",
        trunk: Some("0"),
        nsn_min: 10,
        nsn_max: 10,
        phone_example: "11 4321 0000",
        postal: &["NNNN", "ANNNNAAA"],
        postal_example: "C1425DKE",
    },
    Country {
        code: "AT",
        name: "Austria",
        calling: "43",
        trunk: Some("0"),
        nsn_min: 4,
        nsn_max: 13,
        phone_example: "1 2345678",
        postal: &["NNNN"],
        postal_example: "1010",
    },
    Country {
        code: "AU",
        name: "Australia",
        calling: "61",
        trunk: Some("0"),
        nsn_min: 9,
        nsn_max: 9,
        phone_example: "2 9374 4000",
        postal: &["NNNN"],
        postal_example: "2000",
    },
    Country {
        code: "BE",
        name: "Belgium",
        calling: "32",
        trunk: Some("0"),
        nsn_min: 8,
        nsn_max: 9,
        phone_example: "2 511 21 23",
        postal: &["NNNN"],
        postal_example: "1000",
    },
    Country {
        code: "BR",
        name: "Brazil",
        calling: "55",
        trunk: Some("0"),
        nsn_min: 10,
        nsn_max: 11,
        phone_example: "11 91234 5678",
        postal: &["NNNNN-NNN"],
        postal_example: "01310-100",
    },
    Country {
        code: "CA",
        name: "Canada",
        calling: "1",
        trunk: None,
        nsn_min: 10,
        nsn_max: 10,
        phone_example: "416 555 0132",
        postal: &["ANA NAN"],
        postal_example: "K1A 0B1",
    },
    Country {
        code: "CH",
        name: "Switzerland",
        calling: "41",
        trunk: Some("0"),
        nsn_min: 9,
        nsn_max: 9,
        phone_example: "44 668 18 00",
        postal: &["NNNN"],
        postal_example: "8001",
    },
    Country {
        code: "CN",
        name: "China",
        calling: "86",
        trunk: Some("0"),
        nsn_min: 9,
        nsn_max: 11,
        phone_example: "10 6552 9988",
        postal: &["NNNNNN"],
        postal_example: "100000",
    },
    Country {
        code: "CZ",
        name: "Czechia",
        calling: "420",
        trunk: None,
        nsn_min: 9,
        nsn_max: 9,
        phone_example: "212 812 111",
        postal: &["NNN NN"],
        postal_example: "110 00",
    },
    Country {
        code: "DE",
        name: "Germany",
        calling: "49",
        trunk: Some("0"),
        nsn_min: 6,
        nsn_max: 11,
        phone_example: "30 2360 8000",
        postal: &["NNNNN"],
        postal_example: "10115",
    },
    Country {
        code: "DK",
        name: "Denmark",
        calling: "45",
        trunk: None,
        nsn_min: 8,
        nsn_max: 8,
        phone_example: "32 47 33 00",
        postal: &["NNNN"],
        postal_example: "1050",
    },
    Country {
        code: "ES",
        name: "Spain",
        calling: "34",
        trunk: None,
        nsn_min: 9,
        nsn_max: 9,
        phone_example: "912 345 678",
        postal: &["NNNNN"],
        postal_example: "28013",
    },
    Country {
        code: "FI",
        name: "Finland",
        calling: "358",
        trunk: Some("0"),
        nsn_min: 5,
        nsn_max: 12,
        phone_example: "9 4767 0111",
        postal: &["NNNNN"],
        postal_example: "00100",
    },
    Country {
        code: "FR",
        name: "France",
        calling: "33",
        trunk: Some("0"),
        nsn_min: 9,
        nsn_max: 9,
        phone_example: "1 42 68 53 00",
        postal: &["NNNNN"],
        postal_example: "75008",
    },
    Country {
        code: "GB",
        name: "United Kingdom",
        calling: "44",
        trunk: Some("0"),
        nsn_min: 9,
        nsn_max: 10,
        phone_example: "20 7946 0958",
        postal: GB_POSTAL,
        postal_example: "SW1A 1AA",
    },
    Country {
        code: "GR",
        name: "Greece",
        calling: "30",
        trunk: None,
        nsn_min: 10,
        nsn_max: 10,
        phone_example: "21 0324 9652",
        postal: &["NNN NN"],
        postal_example: "104 31",
    },
    Country {
        code: "HU",
        name: "Hungary",
        calling: "36",
        trunk: Some("06"),
        nsn_min: 8,
        nsn_max: 9,
        phone_example: "1 429 6000",
        postal: &["NNNN"],
        postal_example: "1051",
    },
    Country {
        code: "IE",
        name: "Ireland",
        calling: "353",
        trunk: Some("0"),
        nsn_min: 7,
        nsn_max: 9,
        phone_example: "1 618 1111",
        postal: &["ANN CCCC"],
        postal_example: "D02 AF30",
    },
    Country {
        code: "IL",
        name: "Israel",
        calling: "972",
        trunk: Some("0"),
        nsn_min: 8,
        nsn_max: 9,
        phone_example: "2 629 5555",
        postal: &["NNNNNNN"],
        postal_example: "9103401",
    },
    Country {
        code: "IN",
        name: "India",
        calling: "91",
        trunk: Some("0"),
        nsn_min: 10,
        nsn_max: 10,
        phone_example: "11 2301 0101",
        postal: &["NNNNNN", "NNN NNN"],
        postal_example: "110001",
    },
    Country {
        code: "IT",
        name: "Italy",
        calling: "39",
        trunk: None,
        nsn_min: 6,
        nsn_max: 11,
        phone_example: "06 6982",
        postal: &["NNNNN"],
        postal_example: "00184",
    },
    Country {
        code: "JP",
        name: "Japan",
        calling: "81",
        trunk: Some("0"),
        nsn_min: 9,
        nsn_max: 10,
        phone_example: "3 3201 3331",
        postal: &["NNN-NNNN"],
        postal_example: "100-0001",
    },
    Country {
        code: "KR",
        name: "South Korea",
        calling: "82",
        trunk: Some("0"),
        nsn_min: 8,
        nsn_max: 11,
        phone_example: "2 2075 4000",
        postal: &["NNNNN"],
        postal_example: "03187",
    },
    Country {
        code: "MX",
        name: "Mexico",
        calling: "52",
        trunk: None,
        nsn_min: 10,
        nsn_max: 10,
        phone_example: "55 5080 2000",
        postal: &["NNNNN"],
        postal_example: "06000",
    },
    Country {
        code: "NL",
        name: "Netherlands",
        calling: "31",
        trunk: Some("0"),
        nsn_min: 9,
        nsn_max: 9,
        phone_example: "20 624 1111",
        postal: &["NNNN AA"],
        postal_example: "1012 AB",
    },
    Country {
        code: "NO",
        name: "Norway",
        calling: "47",
        trunk: None,
        nsn_min: 8,
        nsn_max: 8,
        phone_example: "22 82 60 00",
        postal: &["NNNN"],
        postal_example: "0150",
    },
    Country {
        code: "NZ",
        name: "New Zealand",
        calling: "64",
        trunk: Some("0"),
        nsn_min: 8,
        nsn_max: 10,
        phone_example: "4 472 1000",
        postal: &["NNNN"],
        postal_example: "6011",
    },
    Country {
        code: "PL",
        name: "Poland",
        calling: "48",
        trunk: None,
        nsn_min: 9,
        nsn_max: 9,
        phone_example: "22 630 63 04",
        postal: &["NN-NNN"],
        postal_example: "00-001",
    },
    Country {
        code: "PT",
        name: "Portugal",
        calling: "351",
        trunk: None,
        nsn_min: 9,
        nsn_max: 9,
        phone_example: "213 466 141",
        postal: &["NNNN-NNN"],
        postal_example: "1000-260",
    },
    Country {
        code: "RO",
        name: "Romania",
        calling: "40",
        trunk: Some("0"),
        nsn_min: 9,
        nsn_max: 9,
        phone_example: "21 315 5900",
        postal: &["NNNNNN"],
        postal_example: "010101",
    },
    Country {
        code: "RU",
        name: "Russia",
        calling: "7",
        trunk: Some("8"),
        nsn_min: 10,
        nsn_max: 10,
        phone_example: "495 123 4567",
        postal: &["NNNNNN"],
        postal_example: "101000",
    },
    Country {
        code: "SE",
        name: "Sweden",
        calling: "46",
        trunk: Some("0"),
        nsn_min: 7,
        nsn_max: 13,
        phone_example: "8 508 313 00",
        postal: &["NNN NN"],
        postal_example: "114 55",
    },
    Country {
        code: "SG",
        name: "Singapore",
        calling: "65",
        trunk: None,
        nsn_min: 8,
        nsn_max: 8,
        phone_example: "6337 8377",
        postal: &["NNNNNN"],
        postal_example: "018956",
    },
    Country {
        code: "SK",
        name: "Slovakia",
        calling: "421",
        trunk: Some("0"),
        nsn_min: 9,
        nsn_max: 9,
        phone_example: "2 5443 4082",
        postal: &["NNN NN"],
        postal_example: "811 01",
    },
    Country {
        code: "TR",
        name: "Turkey",
        calling: "90",
        trunk: Some("0"),
        nsn_min: 10,
        nsn_max: 10,
        phone_example: "212 293 1300",
        postal: &["NNNNN"],
        postal_example: "34000",
    },
    Country {
        code: "UA",
        name: "Ukraine",
        calling: "380",
        trunk: Some("0"),
        nsn_min: 9,
        nsn_max: 9,
        phone_example: "44 234 5678",
        postal: &["NNNNN"],
        postal_example: "01001",
    },
    Country {
        code: "US",
        name: "United States",
        calling: "1",
        trunk: None,
        nsn_min: 10,
        nsn_max: 10,
        phone_example: "415 555 2671",
        postal: &["NNNNN", "NNNNN-NNNN"],
        postal_example: "90210",
    },
    Country {
        code: "ZA",
        name: "South Africa",
        calling: "27",
        trunk: Some("0"),
        nsn_min: 9,
        nsn_max: 9,
        phone_example: "21 480 7700",
        postal: &["NNNN"],
        postal_example: "0001",
    },
];

fn find_country(code: &str) -> Option<&'static Country> {
    let want = code.trim().to_ascii_uppercase();
    COUNTRIES.iter().find(|c| c.code == want)
}

// ---------------------------------------------------------------------------
// per-type checks
// ---------------------------------------------------------------------------

/// A value that passed its check: the canonical form plus an optional note
/// (currently the detected card brand).
struct Valid {
    normalized: String,
    note: Option<String>,
}

fn ok(normalized: String) -> Result<Valid, String> {
    Ok(Valid {
        normalized,
        note: None,
    })
}

fn validate_email(v: &str) -> Result<Valid, String> {
    let s = v.trim();
    let ats = s.matches('@').count();
    if ats == 0 {
        return Err("missing the \"@\" separator".into());
    }
    if ats > 1 {
        return Err(format!(
            "contains {ats} \"@\" characters; an address has exactly one"
        ));
    }
    if s.len() > 254 {
        return Err(format!(
            "is {} characters long; the limit is 254",
            s.chars().count()
        ));
    }
    let (local, domain) = s.split_once('@').expect("one @ present");
    if local.is_empty() {
        return Err("has nothing before the \"@\"".into());
    }
    if local.len() > 64 {
        return Err(format!(
            "has {} characters before the \"@\"; the limit is 64",
            local.chars().count()
        ));
    }
    if local.starts_with('.') || local.ends_with('.') {
        return Err("starts or ends the part before \"@\" with a dot".into());
    }
    if local.contains("..") {
        return Err("has two dots in a row before the \"@\"".into());
    }
    for ch in local.chars() {
        if !(ch.is_ascii_alphanumeric() || "!#$%&'*+-/=?^_`{|}~.".contains(ch)) {
            return Err(format!(
                "uses \"{ch}\", which is not allowed before the \"@\""
            ));
        }
    }
    if domain.is_empty() {
        return Err("is missing the domain after the \"@\"".into());
    }
    if domain.len() > 253 {
        return Err(format!(
            "has a {}-character domain; the limit is 253",
            domain.chars().count()
        ));
    }
    let labels: Vec<&str> = domain.split('.').collect();
    if labels.len() < 2 {
        return Err(format!("has a domain \"{domain}\" with no dot"));
    }
    for label in &labels {
        if label.is_empty() {
            return Err(
                "has an empty part in the domain (a leading, trailing or doubled dot)".into(),
            );
        }
        if label.len() > 63 {
            return Err(format!(
                "has a domain part \"{label}\" longer than 63 characters"
            ));
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err(format!(
                "has a domain part \"{label}\" starting or ending with a hyphen"
            ));
        }
        for ch in label.chars() {
            if !(ch.is_ascii_alphanumeric() || ch == '-') {
                return Err(format!("uses \"{ch}\", which is not allowed in a domain"));
            }
        }
    }
    let tld = labels.last().expect("at least two labels");
    if tld.len() < 2 || !tld.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(format!(
            "has the top-level domain \"{tld}\"; it must be at least two letters"
        ));
    }
    ok(format!("{local}@{}", domain.to_ascii_lowercase()))
}

fn validate_url(v: &str) -> Result<Valid, String> {
    let s = v.trim();
    if s.chars().any(|c| c.is_whitespace()) {
        return Err("contains a space".into());
    }
    let Some((scheme, rest)) = s.split_once("://") else {
        return Err("is missing the scheme (start it with https://)".into());
    };
    if scheme.is_empty() {
        return Err("has nothing before \"://\"".into());
    }
    let mut sc = scheme.chars();
    let first = sc.next().expect("non-empty scheme");
    if !first.is_ascii_alphabetic() || !sc.all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)) {
        return Err(format!("has an invalid scheme \"{scheme}\""));
    }
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let tail = &rest[authority_end..];
    if authority.is_empty() {
        return Err("is missing the host after the scheme".into());
    }
    // Strip any userinfo, then any :port.
    let hostport = match authority.rsplit_once('@') {
        Some((_, h)) => h,
        None => authority,
    };
    if hostport.is_empty() {
        return Err("is missing the host after the \"@\"".into());
    }
    let (host, port) = if hostport.starts_with('[') {
        match hostport.split_once(']') {
            Some((h, p)) => (format!("{h}]"), p.strip_prefix(':').unwrap_or("")),
            None => return Err("has an unclosed \"[\" in the host".into()),
        }
    } else {
        match hostport.rsplit_once(':') {
            Some((h, p)) => (h.to_string(), p),
            None => (hostport.to_string(), ""),
        }
    };
    if !port.is_empty() {
        match port.parse::<u32>() {
            Ok(p) if p >= 1 && p <= 65535 => {}
            _ => return Err(format!("has an invalid port \"{port}\"")),
        }
    }
    if host.is_empty() {
        return Err("is missing the host before the port".into());
    }
    let is_ipv6 = host.starts_with('[') && host.ends_with(']');
    let is_ipv4 = host.split('.').count() == 4
        && host
            .split('.')
            .all(|o| !o.is_empty() && o.len() <= 3 && o.chars().all(|c| c.is_ascii_digit()));
    if !is_ipv6 && !is_ipv4 && !host.eq_ignore_ascii_case("localhost") {
        let labels: Vec<&str> = host.split('.').collect();
        if labels.len() < 2 {
            return Err(format!("has a host \"{host}\" with no domain suffix"));
        }
        for label in &labels {
            if label.is_empty() {
                return Err(format!("has an empty part in the host \"{host}\""));
            }
            for ch in label.chars() {
                if !(ch.is_ascii_alphanumeric() || ch == '-') {
                    return Err(format!("uses \"{ch}\", which is not allowed in a host"));
                }
            }
        }
        let tld = labels.last().expect("at least two labels");
        if tld.len() < 2 || !tld.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(format!(
                "has the top-level domain \"{tld}\"; it must be at least two letters"
            ));
        }
    }
    let port_part = if port.is_empty() {
        String::new()
    } else {
        format!(":{port}")
    };
    ok(format!(
        "{}://{}{}{}",
        scheme.to_ascii_lowercase(),
        host.to_ascii_lowercase(),
        port_part,
        tail
    ))
}

fn digit_range(c: &Country) -> String {
    if c.nsn_min == c.nsn_max {
        format!("{}", c.nsn_min)
    } else {
        format!("{}-{}", c.nsn_min, c.nsn_max)
    }
}

fn validate_phone(v: &str, country: Option<&Country>) -> Result<Valid, String> {
    let s = v.trim();
    let mut plus = false;
    for (i, ch) in s.chars().enumerate() {
        if ch == '+' {
            if i != 0 {
                return Err("has a \"+\" that is not at the start".into());
            }
            plus = true;
        } else if !(ch.is_ascii_digit() || " -().\u{a0}/".contains(ch)) {
            return Err(format!(
                "uses \"{ch}\", which is not valid in a phone number"
            ));
        }
    }
    let mut digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return Err("has no digits".into());
    }
    let mut intl = plus;
    if !plus && digits.len() > 4 && digits.starts_with("00") {
        digits = digits[2..].to_string();
        intl = true;
    }

    let Some(c) = country else {
        // No locale selected: fall back to the E.164 length window.
        if digits.len() < 7 || digits.len() > 15 {
            return Err(format!(
                "has {} digits; an international number has 7-15",
                digits.len()
            ));
        }
        return ok(if intl { format!("+{digits}") } else { digits });
    };

    let strip_trunk = |n: &str| -> String {
        match c.trunk {
            Some(t) if n.starts_with(t) && n.len() > t.len() => n[t.len()..].to_string(),
            _ => n.to_string(),
        }
    };

    let nsn = if intl {
        if !digits.starts_with(c.calling) {
            let shown: String = digits.chars().take(c.calling.len()).collect();
            return Err(format!(
                "starts with +{shown}, but {} numbers start with +{}",
                c.name, c.calling
            ));
        }
        let rest = digits[c.calling.len()..].to_string();
        // Tolerate the "+44 (0)20 …" habit of writing the trunk prefix too.
        if rest.len() > c.nsn_max {
            strip_trunk(&rest)
        } else {
            rest
        }
    } else {
        // Most-specific reading first: a leading trunk prefix is the strongest
        // signal (a national number never starts with it), then the number as
        // typed, then a country code written without the "+".
        let mut cands: Vec<String> = Vec::new();
        let trunked = strip_trunk(&digits);
        if trunked != digits {
            cands.push(trunked);
        }
        cands.push(digits.clone());
        if digits.len() > c.calling.len() && digits.starts_with(c.calling) {
            let rest = digits[c.calling.len()..].to_string();
            let rest_trunked = strip_trunk(&rest);
            if rest_trunked != rest {
                cands.push(rest_trunked);
            }
            cands.push(rest);
        }
        cands
            .iter()
            .find(|n| n.len() >= c.nsn_min && n.len() <= c.nsn_max)
            .cloned()
            .unwrap_or_else(|| cands[0].clone())
    };

    if nsn.len() < c.nsn_min || nsn.len() > c.nsn_max {
        return Err(format!(
            "has {} national digits; {} numbers have {}",
            nsn.len(),
            c.name,
            digit_range(c)
        ));
    }
    ok(format!("+{}{}", c.calling, nsn))
}

/// Fit a compacted (alphanumeric-only, upper-cased) code to one postal pattern,
/// returning the canonically separated form.
fn fit_pattern(pattern: &str, compact: &str) -> Option<String> {
    let slots: Vec<char> = pattern
        .chars()
        .filter(|c| matches!(c, 'N' | 'A' | 'C'))
        .collect();
    let got: Vec<char> = compact.chars().collect();
    if slots.len() != got.len() {
        return None;
    }
    for (slot, ch) in slots.iter().zip(got.iter()) {
        let fits = match slot {
            'N' => ch.is_ascii_digit(),
            'A' => ch.is_ascii_alphabetic(),
            _ => ch.is_ascii_alphanumeric(),
        };
        if !fits {
            return None;
        }
    }
    let mut out = String::new();
    let mut next = got.iter();
    for ch in pattern.chars() {
        match ch {
            'N' | 'A' | 'C' => out.push(*next.next().expect("length checked")),
            literal => out.push(literal),
        }
    }
    Some(out)
}

/// Human wording for a country's postal shapes, e.g.
/// `NNNNN or NNNNN-NNNN (N = digit)`.
fn postal_format(c: &Country) -> String {
    let mut legend: Vec<&str> = Vec::new();
    let all: String = c.postal.concat();
    if all.contains('N') {
        legend.push("N = digit");
    }
    if all.contains('A') {
        legend.push("A = letter");
    }
    if all.contains('C') {
        legend.push("C = letter or digit");
    }
    format!("{} ({})", c.postal.join(" or "), legend.join(", "))
}

fn validate_postal(v: &str, country: Option<&Country>) -> Result<Valid, String> {
    let s = v.trim();
    for ch in s.chars() {
        if !(ch.is_ascii_alphanumeric() || ch == ' ' || ch == '-') {
            return Err(format!(
                "uses \"{ch}\"; a postal code holds only letters, digits, spaces and hyphens"
            ));
        }
    }
    let compact: String = s
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let Some(c) = country else {
        if compact.len() < 2 || compact.len() > 10 {
            return Err(format!(
                "has {} letters/digits; without a country a postal code must have 2-10",
                compact.len()
            ));
        }
        return ok(compact);
    };
    for pattern in c.postal {
        if let Some(canonical) = fit_pattern(pattern, &compact) {
            return ok(canonical);
        }
    }
    Err(format!("does not match the {} postal format", c.name))
}

fn luhn(digits: &str) -> bool {
    let mut sum = 0u32;
    for (i, ch) in digits.chars().rev().enumerate() {
        let mut d = ch.to_digit(10).unwrap_or(0);
        if i % 2 == 1 {
            d *= 2;
            if d > 9 {
                d -= 9;
            }
        }
        sum += d;
    }
    sum % 10 == 0
}

/// Detect the card brand from the issuer prefix, with the lengths that brand
/// issues. Prefix ranges only — no BIN database, so no issuing bank/country.
fn brand_of(d: &str) -> Option<(&'static str, &'static [usize])> {
    let head = |n: usize| -> u32 { d[..n.min(d.len())].parse::<u32>().unwrap_or(u32::MAX) };
    let p2 = &d[..2.min(d.len())];
    let p4 = &d[..4.min(d.len())];
    if d.starts_with('4') {
        return Some(("Visa", &[13, 16, 19]));
    }
    if (51..=55).contains(&head(2)) || (2221..=2720).contains(&head(4)) {
        return Some(("Mastercard", &[16]));
    }
    if p2 == "34" || p2 == "37" {
        return Some(("American Express", &[15]));
    }
    if matches!(
        p4,
        "5018" | "5020" | "5038" | "5893" | "6304" | "6759" | "6761" | "6762" | "6763"
    ) {
        return Some(("Maestro", &[12, 13, 14, 15, 16, 17, 18, 19]));
    }
    if p4 == "6011" || p2 == "65" || (644..=649).contains(&head(3)) {
        return Some(("Discover", &[16, 17, 18, 19]));
    }
    if (3528..=3589).contains(&head(4)) {
        return Some(("JCB", &[16, 17, 18, 19]));
    }
    if (300..=305).contains(&head(3)) || p4 == "3095" || p2 == "36" || p2 == "38" || p2 == "39" {
        return Some(("Diners Club", &[14, 16, 19]));
    }
    if p2 == "62" {
        return Some(("UnionPay", &[16, 17, 18, 19]));
    }
    None
}

fn join_lengths(lens: &[usize]) -> String {
    match lens.len() {
        0 => String::new(),
        1 => lens[0].to_string(),
        _ => {
            let head: Vec<String> = lens[..lens.len() - 1]
                .iter()
                .map(|n| n.to_string())
                .collect();
            format!("{} or {}", head.join(", "), lens[lens.len() - 1])
        }
    }
}

fn validate_card(v: &str) -> Result<Valid, String> {
    let s = v.trim();
    for ch in s.chars() {
        if !(ch.is_ascii_digit() || ch == ' ' || ch == '-') {
            return Err(format!(
                "uses \"{ch}\"; a card number holds only digits, spaces and hyphens"
            ));
        }
    }
    let d: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    if d.len() < 12 || d.len() > 19 {
        return Err(format!("has {} digits; a card number has 12-19", d.len()));
    }
    let brand = brand_of(&d);
    if let Some((name, lens)) = brand {
        if !lens.contains(&d.len()) {
            return Err(format!(
                "looks like {name}, but {name} numbers have {} digits; this one has {}",
                join_lengths(lens),
                d.len()
            ));
        }
    }
    if !luhn(&d) {
        return Err("fails the Luhn checksum, so a digit is mistyped".into());
    }
    Ok(Valid {
        normalized: d,
        note: Some(
            brand
                .map(|b| b.0)
                .unwrap_or("unrecognised brand")
                .to_string(),
        ),
    })
}

// ---------------------------------------------------------------------------
// expectations ("what should it look like")
// ---------------------------------------------------------------------------

/// `(expected format, a valid example)` for a type under a country — the hint
/// every failing field carries.
fn expectation(kind: Kind, country: Option<&Country>) -> Option<(String, String)> {
    match kind {
        Kind::Email => Some(("local@domain.tld".into(), "name@example.com".into())),
        Kind::Url => Some((
            "scheme://host/path".into(),
            "https://example.com/pricing".into(),
        )),
        Kind::Phone => Some(match country {
            Some(c) => (
                format!("+{} then {} national digits", c.calling, digit_range(c)),
                format!("+{} {}", c.calling, c.phone_example),
            ),
            None => (
                "+ then 7-15 digits (E.164)".into(),
                "+1 415 555 2671".into(),
            ),
        }),
        Kind::PostalCode => Some(match country {
            Some(c) => (postal_format(c), c.postal_example.to_string()),
            None => ("2-10 letters or digits".into(), "90210".into()),
        }),
        Kind::CreditCard => Some((
            "12-19 digits that pass the Luhn check".into(),
            "4111 1111 1111 1111".into(),
        )),
        Kind::Text => None,
    }
}

// ---------------------------------------------------------------------------
// input parsing
// ---------------------------------------------------------------------------

fn parse_fields(input: &str) -> Result<Vec<(String, String)>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(
            "no fields supplied - give one `name: value` per line, or a JSON object".into(),
        );
    }
    let mut out: Vec<(String, String)> = Vec::new();
    if trimmed.starts_with('{') {
        let value: Value = serde_json::from_str(trimmed)
            .map_err(|e| format!("fields looks like JSON but could not be parsed: {e}"))?;
        let Value::Object(map) = value else {
            return Err("fields JSON must be an object of name/value pairs".into());
        };
        for (k, v) in map {
            let text = match v {
                Value::Null => String::new(),
                Value::String(s) => s,
                other => other.to_string(),
            };
            out.push((k, text));
        }
    } else {
        for line in trimmed.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let colon = line.find(':');
            let equals = line.find('=');
            let cut = match (colon, equals) {
                (Some(c), Some(e)) => Some(c.min(e)),
                (Some(c), None) => Some(c),
                (None, Some(e)) => Some(e),
                (None, None) => None,
            };
            let Some(cut) = cut else {
                return Err(format!(
                    "line \"{line}\" has no separator - write it as `name: value`"
                ));
            };
            let name = line[..cut].trim().to_string();
            let value = line[cut + 1..].trim().to_string();
            if name.is_empty() {
                return Err(format!("line \"{line}\" has an empty field name"));
            }
            out.push((name, value));
        }
    }
    if out.is_empty() {
        return Err(
            "no fields supplied - give one `name: value` per line, or a JSON object".into(),
        );
    }
    if out.len() > MAX_FIELDS {
        return Err(format!(
            "{} fields supplied; the limit is {MAX_FIELDS}",
            out.len()
        ));
    }
    Ok(out)
}

fn parse_rules(input: &str) -> Result<Vec<(String, Kind)>, String> {
    let mut out = Vec::new();
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let colon = line.find(':');
        let equals = line.find('=');
        let cut = match (colon, equals) {
            (Some(c), Some(e)) => Some(c.min(e)),
            (Some(c), None) => Some(c),
            (None, Some(e)) => Some(e),
            (None, None) => None,
        };
        let Some(cut) = cut else {
            return Err(format!(
                "rule \"{line}\" has no separator - write it as `field: type`"
            ));
        };
        let name = line[..cut].trim().to_ascii_lowercase();
        let type_text = line[cut + 1..].trim();
        if name.is_empty() {
            return Err(format!("rule \"{line}\" has an empty field name"));
        }
        let Some(kind) = Kind::parse(type_text) else {
            return Err(format!(
                "rule \"{line}\" has unknown type \"{type_text}\"; use email, phone, url, postal-code, credit-card or text"
            ));
        };
        out.push((name, kind));
    }
    Ok(out)
}

fn parse_required(input: &str) -> (bool, Vec<String>) {
    let mut all = false;
    let mut names = Vec::new();
    for token in input.split([',', ';', '\n', '\r', '\t', ' ']) {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        if token == "*" {
            all = true;
            continue;
        }
        let lowered = token.to_ascii_lowercase();
        if !names.contains(&lowered) {
            names.push(lowered);
        }
    }
    (all, names)
}

// ---------------------------------------------------------------------------
// result model + rendering
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Ok,
    Fail,
    Skip,
}

impl Status {
    fn padded(self) -> &'static str {
        match self {
            Status::Ok => "OK  ",
            Status::Fail => "FAIL",
            Status::Skip => "SKIP",
        }
    }
    fn json(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Fail => "fail",
            Status::Skip => "skipped",
        }
    }
}

struct Entry {
    name: String,
    kind: Kind,
    status: Status,
    /// The value as supplied (masked for card fields when masking is on).
    display: String,
    /// The canonical value (masked the same way), when the check passed.
    normalized: Option<String>,
    note: Option<String>,
    error: Option<String>,
    missing: bool,
}

/// Replace every alphanumeric character except the last four with `*`, keeping
/// separators, so a card number never round-trips into a log or a screenshot.
fn mask_value(raw: &str) -> String {
    let alnum: Vec<usize> = raw
        .char_indices()
        .filter(|(_, c)| c.is_ascii_alphanumeric())
        .map(|(i, _)| i)
        .collect();
    let keep_from = alnum.len().saturating_sub(4);
    let keep: Vec<usize> = alnum[keep_from..].to_vec();
    raw.char_indices()
        .map(|(i, c)| {
            if c.is_ascii_alphanumeric() && !keep.contains(&i) {
                '*'
            } else {
                c
            }
        })
        .collect()
}

fn shown(raw: &str, kind: Kind, mask: bool) -> String {
    if mask && kind == Kind::CreditCard {
        mask_value(raw)
    } else {
        raw.to_string()
    }
}

/// Validate a form submission and render the report.
///
/// * `fields` — `name: value` lines, or a JSON object of name/value pairs.
/// * `country` — ISO 3166-1 alpha-2 code, or `any` for no locale rules.
/// * `required_fields` — names that must carry a value, or `*` for all.
/// * `rules` — `field: type` overrides for the name-based type inference.
/// * `normalize` — report the canonical form of each passing value.
/// * `mask_sensitive` — show card numbers with only the last 4 digits.
/// * `output` — `text` or `json`.
pub fn run(
    fields: &str,
    country: &str,
    required_fields: &str,
    rules: &str,
    normalize: bool,
    mask_sensitive: bool,
    output: &str,
) -> Result<String, String> {
    let country_key = {
        let c = country.trim();
        if c.is_empty() {
            "any"
        } else {
            c
        }
    };
    let selected: Option<&Country> = if country_key.eq_ignore_ascii_case("any") {
        None
    } else {
        Some(find_country(country_key).ok_or_else(|| {
            format!(
                "unknown country \"{country_key}\" - use `any` or an ISO 3166-1 alpha-2 code such as US, GB, DE"
            )
        })?)
    };
    let output_kind = {
        let o = output.trim().to_ascii_lowercase();
        if o.is_empty() {
            "text".to_string()
        } else {
            o
        }
    };
    if output_kind != "text" && output_kind != "json" {
        return Err(format!(
            "unknown output \"{output_kind}\" - use text or json"
        ));
    }

    let parsed = parse_fields(fields)?;
    let rule_map = parse_rules(rules)?;
    let (require_all, required_names) = parse_required(required_fields);

    let mut entries: Vec<Entry> = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    for (name, value) in &parsed {
        let lowered = name.to_ascii_lowercase();
        seen.push(lowered.clone());
        let kind = rule_map
            .iter()
            .find(|(n, _)| *n == lowered)
            .map(|(_, k)| *k)
            .unwrap_or_else(|| Kind::infer(name));
        let is_required = require_all || required_names.contains(&lowered);
        let trimmed = value.trim();
        if trimmed.is_empty() {
            entries.push(Entry {
                name: name.clone(),
                kind,
                status: if is_required {
                    Status::Fail
                } else {
                    Status::Skip
                },
                display: String::new(),
                normalized: None,
                note: None,
                error: if is_required {
                    Some("is required, but no value was supplied".into())
                } else {
                    None
                },
                missing: is_required,
            });
            continue;
        }
        let checked = match kind {
            Kind::Email => validate_email(trimmed),
            Kind::Phone => validate_phone(trimmed, selected),
            Kind::Url => validate_url(trimmed),
            Kind::PostalCode => validate_postal(trimmed, selected),
            Kind::CreditCard => validate_card(trimmed),
            Kind::Text => ok(trimmed.to_string()),
        };
        let display = shown(trimmed, kind, mask_sensitive);
        match checked {
            Ok(v) => {
                let normalized = shown(&v.normalized, kind, mask_sensitive);
                // A masked card's "as typed" form only differs in grouping, so
                // show the canonical masked value for both and skip the noise.
                let display = if kind == Kind::CreditCard && mask_sensitive {
                    normalized.clone()
                } else {
                    display
                };
                entries.push(Entry {
                    name: name.clone(),
                    kind,
                    status: Status::Ok,
                    display,
                    normalized: Some(normalized),
                    note: v.note,
                    error: None,
                    missing: false,
                })
            }
            Err(e) => entries.push(Entry {
                name: name.clone(),
                kind,
                status: Status::Fail,
                display,
                normalized: None,
                note: None,
                error: Some(e),
                missing: false,
            }),
        }
    }

    // Required names that never appeared in the submission at all.
    for want in &required_names {
        if seen.contains(want) {
            continue;
        }
        entries.push(Entry {
            name: want.clone(),
            kind: Kind::infer(want),
            status: Status::Fail,
            display: String::new(),
            normalized: None,
            note: None,
            error: Some("is required, but the form did not include it".into()),
            missing: true,
        });
    }

    let failed = entries.iter().filter(|e| e.status == Status::Fail).count();
    let passed = entries.iter().filter(|e| e.status == Status::Ok).count();
    let skipped = entries.iter().filter(|e| e.status == Status::Skip).count();

    if output_kind == "json" {
        return Ok(render_json(
            &entries, selected, failed, passed, skipped, normalize,
        ));
    }
    Ok(render_text(
        &entries, selected, failed, passed, skipped, normalize,
    ))
}

fn country_label(selected: Option<&Country>) -> String {
    match selected {
        Some(c) => format!("{} ({})", c.code, c.name),
        None => "any (no country-specific rules)".to_string(),
    }
}

fn render_text(
    entries: &[Entry],
    selected: Option<&Country>,
    failed: usize,
    passed: usize,
    skipped: usize,
    normalize: bool,
) -> String {
    let total = entries.len();
    let skipped_part = if skipped > 0 {
        format!(", {skipped} skipped (blank, optional)")
    } else {
        String::new()
    };
    let mut out = format!(
        "{} — {total} field(s) checked: {passed} passed, {failed} failed{skipped_part}. Country: {}.\n",
        if failed == 0 { "VALID" } else { "INVALID" },
        country_label(selected)
    );
    for e in entries {
        let head = format!("{} {} [{}]", e.status.padded(), e.name, e.kind.as_str());
        match e.status {
            Status::Ok => {
                let value = match (&e.normalized, normalize) {
                    (Some(n), true) => n.clone(),
                    _ => e.display.clone(),
                };
                let mut notes: Vec<String> = Vec::new();
                if let Some(n) = &e.note {
                    notes.push(n.clone());
                }
                if normalize {
                    if let Some(n) = &e.normalized {
                        if *n != e.display {
                            notes.push(format!("was \"{}\"", e.display));
                        }
                    }
                }
                let suffix = if notes.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", notes.join("; "))
                };
                out.push_str(&format!("{head} = {value}{suffix}\n"));
            }
            Status::Skip => {
                out.push_str(&format!("{head} — no value supplied (not required)\n"));
            }
            Status::Fail => {
                let error = e.error.clone().unwrap_or_default();
                let value_part = if e.missing {
                    String::new()
                } else {
                    format!(" \"{}\"", e.display)
                };
                let hint = match expectation(e.kind, selected) {
                    Some((fmt, example)) => {
                        format!(" Expected format: {fmt}. Example: {example}.")
                    }
                    None => String::new(),
                };
                out.push_str(&format!("{head}{value_part} — {error}.{hint}\n"));
            }
        }
    }
    out
}

fn render_json(
    entries: &[Entry],
    selected: Option<&Country>,
    failed: usize,
    passed: usize,
    skipped: usize,
    normalize: bool,
) -> String {
    let mut list: Vec<Value> = Vec::new();
    for e in entries {
        let mut o = Map::new();
        o.insert("name".into(), json!(e.name));
        o.insert("type".into(), json!(e.kind.as_str()));
        o.insert("status".into(), json!(e.status.json()));
        o.insert("value".into(), json!(e.display));
        if normalize {
            if let Some(n) = &e.normalized {
                o.insert("normalized".into(), json!(n));
            }
        }
        if let Some(n) = &e.note {
            o.insert("brand".into(), json!(n));
        }
        if let Some(err) = &e.error {
            o.insert("error".into(), json!(err));
            if let Some((fmt, example)) = expectation(e.kind, selected) {
                o.insert("expected_format".into(), json!(fmt));
                o.insert("example".into(), json!(example));
            }
        }
        list.push(Value::Object(o));
    }
    let mut root = Map::new();
    root.insert("valid".into(), json!(failed == 0));
    root.insert(
        "country".into(),
        json!(selected.map(|c| c.code).unwrap_or("any")),
    );
    if let Some(c) = selected {
        root.insert("country_name".into(), json!(c.name));
    }
    root.insert("checked".into(), json!(entries.len()));
    root.insert("passed".into(), json!(passed));
    root.insert("failed".into(), json!(failed));
    root.insert("skipped".into(), json!(skipped));
    root.insert("fields".into(), Value::Array(list));
    serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_us_form_passes_every_field() {
        let out = run(
            "email: John.Doe@Example.COM\nphone: (415) 555-2671\nzip: 90210\nwebsite: https://example.com",
            "US",
            "email",
            "",
            true,
            true,
            "text",
        )
        .unwrap();
        assert!(
            out.starts_with(
                "VALID — 4 field(s) checked: 4 passed, 0 failed. Country: US (United States).\n"
            ),
            "{out}"
        );
        assert!(
            out.contains("OK   email [email] = John.Doe@example.com"),
            "{out}"
        );
        assert!(out.contains("OK   phone [phone] = +14155552671"), "{out}");
    }

    #[test]
    fn broken_us_form_reports_each_field_with_a_hint() {
        let out = run(
            "email: john@\nphone: 555-12\nzip: 9021",
            "US",
            "",
            "",
            true,
            true,
            "text",
        )
        .unwrap();
        assert!(
            out.starts_with("INVALID — 3 field(s) checked: 0 passed, 3 failed."),
            "{out}"
        );
        assert!(
            out.contains("FAIL email [email] \"john@\" — is missing the domain after the \"@\". Expected format: local@domain.tld. Example: name@example.com."),
            "{out}"
        );
        assert!(
            out.contains("FAIL phone [phone] \"555-12\" — has 5 national digits; United States numbers have 10."),
            "{out}"
        );
        assert!(
            out.contains("Expected format: NNNNN or NNNNN-NNNN (N = digit). Example: 90210."),
            "{out}"
        );
    }

    #[test]
    fn country_switches_postal_and_phone_rules() {
        let uk = run(
            "postcode: sw1a1aa\nphone: 020 7946 0958",
            "GB",
            "",
            "",
            true,
            true,
            "text",
        )
        .unwrap();
        assert!(uk.contains("= SW1A 1AA"), "{uk}");
        assert!(uk.contains("= +442079460958"), "{uk}");
        // The same postcode is not a US ZIP.
        let us = run("postcode: SW1A 1AA", "US", "", "", true, true, "text").unwrap();
        assert!(
            us.contains("does not match the United States postal format"),
            "{us}"
        );
        // Canada normalises its own spacing.
        let ca = run("zip: k1a0b1", "CA", "", "", true, true, "text").unwrap();
        assert!(ca.contains("= K1A 0B1"), "{ca}");
    }

    #[test]
    fn card_is_luhn_checked_branded_and_masked() {
        let good = run(
            "card: 4111 1111 1111 1111",
            "any",
            "",
            "",
            true,
            true,
            "text",
        )
        .unwrap();
        assert!(
            good.contains("OK   card [credit-card] = ************1111 (Visa)"),
            "{good}"
        );
        assert!(
            !good.contains("4111"),
            "the full number must never be echoed: {good}"
        );
        let bad = run(
            "card: 4111 1111 1111 1112",
            "any",
            "",
            "",
            true,
            true,
            "text",
        )
        .unwrap();
        assert!(bad.contains("fails the Luhn checksum"), "{bad}");
        let unmasked = run("card: 4111111111111111", "any", "", "", true, false, "text").unwrap();
        assert!(unmasked.contains("= 4111111111111111 (Visa)"), "{unmasked}");
        let amex = run(
            "card: 3782 822463 10005",
            "any",
            "",
            "",
            true,
            false,
            "text",
        )
        .unwrap();
        assert!(
            amex.contains("= 378282246310005 (American Express; was \"3782 822463 10005\")"),
            "{amex}"
        );
        // Right brand prefix, wrong length for that brand.
        let short_visa = run("card: 41111111111111", "any", "", "", true, false, "text").unwrap();
        assert!(
            short_visa.contains("looks like Visa, but Visa numbers have 13, 16 or 19 digits"),
            "{short_visa}"
        );
    }

    #[test]
    fn required_covers_blank_and_absent_fields() {
        let out = run(
            "email: a@b.com\nnickname:",
            "any",
            "nickname, phone",
            "",
            true,
            true,
            "text",
        )
        .unwrap();
        assert!(
            out.contains("FAIL nickname [text] — is required, but no value was supplied."),
            "{out}"
        );
        assert!(
            out.contains("FAIL phone [phone] — is required, but the form did not include it."),
            "{out}"
        );
        // Blank and optional is skipped, not failed.
        let skipped = run(
            "email: a@b.com\nnickname:",
            "any",
            "",
            "",
            true,
            true,
            "text",
        )
        .unwrap();
        assert!(
            skipped.contains("SKIP nickname [text] — no value supplied (not required)"),
            "{skipped}"
        );
        assert!(
            skipped.starts_with("VALID — 2 field(s) checked: 1 passed, 0 failed, 1 skipped"),
            "{skipped}"
        );
    }

    #[test]
    fn rules_override_the_name_based_type() {
        let inferred = run("contact: 4155552671", "US", "", "", true, true, "text").unwrap();
        assert!(inferred.contains("[text]"), "{inferred}");
        let overridden = run(
            "contact: 4155552671",
            "US",
            "",
            "contact: phone",
            true,
            true,
            "text",
        )
        .unwrap();
        assert!(
            overridden.contains("OK   contact [phone] = +14155552671"),
            "{overridden}"
        );
    }

    #[test]
    fn json_output_is_machine_readable() {
        let out = run(
            r#"{"email": "john@", "zip": "90210"}"#,
            "US",
            "",
            "",
            true,
            true,
            "json",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["valid"], json!(false));
        assert_eq!(v["country"], json!("US"));
        assert_eq!(v["checked"], json!(2));
        assert_eq!(v["failed"], json!(1));
        let email = v["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["name"] == json!("email"))
            .unwrap()
            .clone();
        assert_eq!(email["status"], json!("fail"));
        assert_eq!(email["expected_format"], json!("local@domain.tld"));
    }

    #[test]
    fn url_errors_name_the_missing_part() {
        let out = run(
            "site: example.com\nbackup: https://\nport: https://example.com:99999/",
            "any",
            "",
            "site: url\nbackup: url\nport: url",
            true,
            true,
            "text",
        )
        .unwrap();
        assert!(
            out.contains("is missing the scheme (start it with https://)"),
            "{out}"
        );
        assert!(
            out.contains("is missing the host after the scheme"),
            "{out}"
        );
        assert!(out.contains("has an invalid port \"99999\""), "{out}");
    }

    #[test]
    fn phone_accepts_plus_trunk_and_bare_national_forms() {
        for value in [
            "+1 415 555 2671",
            "001 415 555 2671",
            "(415) 555-2671",
            "1-415-555-2671",
        ] {
            let out = run(&format!("phone: {value}"), "US", "", "", true, true, "text").unwrap();
            assert!(out.contains("= +14155552671"), "{value}: {out}");
        }
        // A German number written with the national trunk prefix.
        let de = run("phone: 030 23608000", "DE", "", "", true, true, "text").unwrap();
        assert!(de.contains("= +493023608000"), "{de}");
        // Wrong calling code for the selected country.
        let wrong = run("phone: +33 1 42 68 53 00", "US", "", "", true, true, "text").unwrap();
        assert!(
            wrong.contains("starts with +3, but United States numbers start with +1"),
            "{wrong}"
        );
    }

    #[test]
    fn errors_on_bad_inputs() {
        assert!(run("", "US", "", "", true, true, "text").is_err());
        assert!(run("email: a@b.com", "ZZ", "", "", true, true, "text")
            .unwrap_err()
            .contains("unknown country"));
        assert!(run(
            "email: a@b.com",
            "US",
            "",
            "email: colour",
            true,
            true,
            "text"
        )
        .unwrap_err()
        .contains("unknown type"));
        assert!(run("email: a@b.com", "US", "", "", true, true, "yaml")
            .unwrap_err()
            .contains("unknown output"));
        assert!(run("just a line", "US", "", "", true, true, "text")
            .unwrap_err()
            .contains("no separator"));
        let many: String = (0..MAX_FIELDS + 1).map(|i| format!("f{i}: x\n")).collect();
        assert!(run(&many, "US", "", "", true, true, "text")
            .unwrap_err()
            .contains("the limit is 200"));
    }

    #[test]
    fn country_codes_match_the_country_table() {
        assert_eq!(COUNTRY_CODES[0], "any");
        assert_eq!(COUNTRY_CODES.len(), COUNTRIES.len() + 1);
        for (code, country) in COUNTRY_CODES[1..].iter().zip(COUNTRIES.iter()) {
            assert_eq!(*code, country.code);
        }
        // Every country's own example passes its own rules.
        for c in COUNTRIES {
            let form = format!("zip: {}\nphone: {}", c.postal_example, c.phone_example);
            let out = run(&form, c.code, "", "", true, true, "text").unwrap();
            assert!(out.starts_with("VALID"), "{}: {out}", c.code);
        }
    }
}
