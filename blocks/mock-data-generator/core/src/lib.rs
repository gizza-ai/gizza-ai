//! mock-data-generator core — turn a compact shorthand SCHEMA (your own field
//! names + types, with array counts and nested objects) into realistic mock
//! JSON. Pure Rust, no I/O — shared by the chat skill block and the web page.
//!
//! Unlike a fixed-column fake-data generator, the caller defines the *shape*:
//! arbitrary field names mapped to a type token, optional `[n]` array repeat,
//! and `{ ... }` nested objects. Output is JSON (a single object, or an array
//! of `count` objects). Deterministic given a non-zero `seed`.
//!
//! Shorthand grammar (whitespace-insensitive, fields separated by commas or
//! newlines):
//!   field      := NAME ':' TYPESPEC
//!   TYPESPEC   := TYPE ('[' INT ']')?            // optional array repeat
//!   TYPE       := builtin | object
//!   object     := '{' field (',' field)* '}'
//!   builtin    := string | int | float | bool | uuid | name | first_name |
//!                 last_name | username | email | phone | date | datetime |
//!                 url | domain | word | words | sentence | paragraph | city |
//!                 state | country | zip | street | color | ipv4 | mac |
//!                 lat | lng | enum(a|b|c) | int(min..max) | float(min..max)
//!
//! Example:  `id:int(1..1000), user:{name:name, email:email}, tags:string[3]`

pub const MAX_COUNT: u32 = 1000;
pub const MAX_FIELDS: usize = 200; // total fields across the whole (nested) schema
pub const MAX_ARRAY: u32 = 1000;
pub const MAX_DEPTH: usize = 8;

const FIRST_NAMES: &[&str] = &[
    "Ava", "Liam", "Mia", "Noah", "Emma", "Oliver", "Sophia", "Lucas", "Isabella", "Mason",
    "Amelia", "Ethan", "Harper", "James", "Evelyn", "Ben", "Aria", "Leo", "Nora", "Owen",
    "Zoe", "Caleb", "Lily", "Max", "Ruby", "Theo", "Ivy", "Jack", "Maya", "Felix",
];
const LAST_NAMES: &[&str] = &[
    "Smith", "Jones", "Brown", "Taylor", "Wilson", "Davies", "Evans", "Thomas", "Roberts",
    "Walker", "Wright", "Green", "Hall", "Clark", "Patel", "Khan", "Lee", "Young", "King",
    "Scott", "Adams", "Baker", "Carter", "Ford", "Gray", "Hill", "Nash", "Reed", "Stone", "Webb",
];
const DOMAINS: &[&str] = &["example.com", "test.org", "sample.net", "demo.io", "mock.dev"];
const CITIES: &[&str] = &[
    "Springfield", "Riverton", "Fairview", "Lakewood", "Madison", "Georgetown", "Clinton",
    "Franklin", "Greenville", "Bristol", "Ashland", "Dover", "Newport", "Oxford", "Salem",
];
const STATES: &[&str] = &[
    "CA", "NY", "TX", "FL", "WA", "IL", "PA", "OH", "GA", "NC", "MI", "AZ", "MA", "CO", "OR",
];
const COUNTRIES: &[&str] = &[
    "United States", "Canada", "United Kingdom", "Germany", "France", "Spain", "Italy", "Japan",
    "Australia", "Brazil", "India", "Netherlands", "Sweden", "Mexico", "Ireland",
];
const STREETS: &[&str] = &[
    "Main St", "Oak Ave", "Pine Rd", "Maple Dr", "Cedar Ln", "Elm St", "Park Ave", "Hill Rd",
    "Lake Dr", "River Rd", "Sunset Blvd", "Birch Way", "Spruce Ct", "Willow Ln", "Ash St",
];
const WORDS: &[&str] = &[
    "lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing", "elit", "sed", "do",
    "eiusmod", "tempor", "incididunt", "ut", "labore", "magna", "aliqua", "enim", "minim",
    "veniam", "quis", "nostrud", "exercitation", "ullamco", "laboris", "nisi", "aliquip", "ex",
    "commodo", "consequat", "duis", "aute", "irure", "reprehenderit", "voluptate", "velit",
];

/// Tiny deterministic PRNG (splitmix64) — no external dep, stable output.
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in [0, n) for n > 0.
    fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next_u64() % n
        }
    }
    fn pick<'a>(&mut self, items: &'a [&'a str]) -> &'a str {
        items[self.below(items.len() as u64) as usize]
    }
    /// Uniform f64 in [0, 1).
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range_i(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            lo
        } else {
            let span = (hi - lo + 1) as u64;
            lo + self.below(span) as i64
        }
    }
    fn range_f(&mut self, lo: f64, hi: f64) -> f64 {
        if hi <= lo {
            lo
        } else {
            lo + self.unit() * (hi - lo)
        }
    }
    fn bool(&mut self) -> bool {
        self.next_u64() & 1 == 0
    }
}

/// A parsed field type.
enum Ty {
    StringRand,
    IntDefault,
    FloatDefault,
    Bool,
    Uuid,
    FirstName,
    LastName,
    Name,
    Username,
    Email,
    Phone,
    Date,
    DateTime,
    Url,
    Domain,
    Word,
    Words(u32),
    Sentence,
    Paragraph,
    City,
    State,
    Country,
    Zip,
    Street,
    Color,
    Ipv4,
    Mac,
    Lat,
    Lng,
    Enum(Vec<String>),
    IntRange(i64, i64),
    FloatRange(f64, f64),
    Object(Vec<Field>),
}

struct Field {
    name: String,
    ty: Ty,
    array: Option<u32>, // Some(n) → array of n values
}

/// Public entry: produce JSON for `schema`. If `count` > 1 the top-level result
/// is an array of `count` records; otherwise a single object.
pub fn generate(schema: &str, count: u32, seed: u64, pretty: bool) -> Result<String, String> {
    if count == 0 {
        return Err("count must be at least 1".into());
    }
    if count > MAX_COUNT {
        return Err(format!("count must be {MAX_COUNT} or fewer"));
    }
    let mut field_budget = MAX_FIELDS;
    let fields = parse_object_body(schema, &mut field_budget, 0)?;
    if fields.is_empty() {
        return Err("schema is empty — provide at least one `name:type` field".into());
    }
    let mut rng = Rng::new(if seed == 0 { 0xC0FF_EE12_3456_789A } else { seed });

    let mut out = String::new();
    if count > 1 {
        out.push('[');
        for i in 0..count {
            if i > 0 {
                out.push(',');
            }
            if pretty {
                out.push('\n');
                push_indent(&mut out, 1);
            }
            render_object(&fields, &mut rng, &mut out, pretty, 1);
        }
        if pretty {
            out.push('\n');
        }
        out.push(']');
    } else {
        render_object(&fields, &mut rng, &mut out, pretty, 0);
    }
    if pretty {
        out.push('\n');
    }
    Ok(out)
}

fn push_indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn render_object(fields: &[Field], rng: &mut Rng, out: &mut String, pretty: bool, depth: usize) {
    out.push('{');
    for (i, f) in fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        if pretty {
            out.push('\n');
            push_indent(out, depth + 1);
        }
        push_json_string(out, &f.name);
        out.push(':');
        if pretty {
            out.push(' ');
        }
        match f.array {
            Some(n) => {
                out.push('[');
                for j in 0..n {
                    if j > 0 {
                        out.push(',');
                    }
                    if pretty {
                        out.push('\n');
                        push_indent(out, depth + 2);
                    }
                    render_value(&f.ty, rng, out, pretty, depth + 2);
                }
                if pretty && n > 0 {
                    out.push('\n');
                    push_indent(out, depth + 1);
                }
                out.push(']');
            }
            None => render_value(&f.ty, rng, out, pretty, depth + 1),
        }
    }
    if pretty && !fields.is_empty() {
        out.push('\n');
        push_indent(out, depth);
    }
    out.push('}');
}

fn render_value(ty: &Ty, rng: &mut Rng, out: &mut String, pretty: bool, depth: usize) {
    match ty {
        Ty::Object(inner) => render_object(inner, rng, out, pretty, depth),
        Ty::Bool => out.push_str(if rng.bool() { "true" } else { "false" }),
        Ty::IntDefault => out.push_str(&rng.range_i(0, 9999).to_string()),
        Ty::IntRange(lo, hi) => out.push_str(&rng.range_i(*lo, *hi).to_string()),
        Ty::FloatDefault => {
            let v = (rng.range_f(0.0, 1000.0) * 100.0).round() / 100.0;
            out.push_str(&fmt_f64(v));
        }
        Ty::FloatRange(lo, hi) => {
            let v = (rng.range_f(*lo, *hi) * 100.0).round() / 100.0;
            out.push_str(&fmt_f64(v));
        }
        Ty::Lat => {
            let v = (rng.range_f(-90.0, 90.0) * 1e6).round() / 1e6;
            out.push_str(&fmt_f64(v));
        }
        Ty::Lng => {
            let v = (rng.range_f(-180.0, 180.0) * 1e6).round() / 1e6;
            out.push_str(&fmt_f64(v));
        }
        // Everything else is a JSON string.
        _ => {
            let s = render_string(ty, rng);
            push_json_string(out, &s);
        }
    }
}

fn fmt_f64(v: f64) -> String {
    let mut s = format!("{v}");
    if !s.contains('.') && !s.contains('e') && !s.contains('E') {
        s.push_str(".0");
    }
    s
}

fn render_string(ty: &Ty, rng: &mut Rng) -> String {
    match ty {
        Ty::StringRand => {
            let n = 1 + rng.below(2) as usize;
            (0..n)
                .map(|_| rng.pick(WORDS))
                .collect::<Vec<_>>()
                .join("-")
        }
        Ty::Uuid => uuid(rng),
        Ty::FirstName => rng.pick(FIRST_NAMES).to_string(),
        Ty::LastName => rng.pick(LAST_NAMES).to_string(),
        Ty::Name => format!("{} {}", rng.pick(FIRST_NAMES), rng.pick(LAST_NAMES)),
        Ty::Username => {
            let f = rng.pick(FIRST_NAMES).to_ascii_lowercase();
            let l = rng.pick(LAST_NAMES).to_ascii_lowercase();
            format!("{f}.{l}{}", rng.below(100))
        }
        Ty::Email => {
            let f = rng.pick(FIRST_NAMES).to_ascii_lowercase();
            let l = rng.pick(LAST_NAMES).to_ascii_lowercase();
            let d = rng.pick(DOMAINS);
            format!("{f}.{l}@{d}")
        }
        Ty::Phone => format!(
            "+1-{:03}-{:03}-{:04}",
            rng.range_i(200, 999),
            rng.range_i(200, 999),
            rng.range_i(0, 9999)
        ),
        Ty::Date => {
            let y = rng.range_i(1970, 2024);
            let m = rng.range_i(1, 12);
            let d = rng.range_i(1, 28);
            format!("{y:04}-{m:02}-{d:02}")
        }
        Ty::DateTime => {
            let y = rng.range_i(1970, 2024);
            let m = rng.range_i(1, 12);
            let d = rng.range_i(1, 28);
            let hh = rng.range_i(0, 23);
            let mm = rng.range_i(0, 59);
            let ss = rng.range_i(0, 59);
            format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
        }
        Ty::Url => format!("https://{}/{}", rng.pick(DOMAINS), rng.pick(WORDS)),
        Ty::Domain => rng.pick(DOMAINS).to_string(),
        Ty::Word => rng.pick(WORDS).to_string(),
        Ty::Words(n) => (0..(*n).max(1))
            .map(|_| rng.pick(WORDS))
            .collect::<Vec<_>>()
            .join(" "),
        Ty::Sentence => sentence(rng),
        Ty::Paragraph => {
            let n = 3 + rng.below(3);
            (0..n).map(|_| sentence(rng)).collect::<Vec<_>>().join(" ")
        }
        Ty::City => rng.pick(CITIES).to_string(),
        Ty::State => rng.pick(STATES).to_string(),
        Ty::Country => rng.pick(COUNTRIES).to_string(),
        Ty::Zip => format!("{:05}", rng.range_i(10000, 99999)),
        Ty::Street => format!("{} {}", rng.range_i(1, 9999), rng.pick(STREETS)),
        Ty::Color => format!(
            "#{:02x}{:02x}{:02x}",
            rng.below(256),
            rng.below(256),
            rng.below(256)
        ),
        Ty::Ipv4 => format!(
            "{}.{}.{}.{}",
            rng.below(256),
            rng.below(256),
            rng.below(256),
            rng.below(256)
        ),
        Ty::Mac => format!(
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            rng.below(256),
            rng.below(256),
            rng.below(256),
            rng.below(256),
            rng.below(256),
            rng.below(256)
        ),
        Ty::Enum(opts) => {
            if opts.is_empty() {
                String::new()
            } else {
                opts[rng.below(opts.len() as u64) as usize].clone()
            }
        }
        // numeric/object types never reach here
        _ => String::new(),
    }
}

fn sentence(rng: &mut Rng) -> String {
    let n = 5 + rng.below(7);
    let mut words: Vec<String> = (0..n).map(|_| rng.pick(WORDS).to_string()).collect();
    if let Some(first) = words.first_mut() {
        let mut c = first.chars();
        if let Some(f) = c.next() {
            *first = f.to_uppercase().collect::<String>() + c.as_str();
        }
    }
    format!("{}.", words.join(" "))
}

fn uuid(rng: &mut Rng) -> String {
    let a = rng.next_u64();
    let b = rng.next_u64();
    let mut by = [
        (a >> 56) as u8,
        (a >> 48) as u8,
        (a >> 40) as u8,
        (a >> 32) as u8,
        (a >> 24) as u8,
        (a >> 16) as u8,
        (a >> 8) as u8,
        a as u8,
        (b >> 56) as u8,
        (b >> 48) as u8,
        (b >> 40) as u8,
        (b >> 32) as u8,
        (b >> 24) as u8,
        (b >> 16) as u8,
        (b >> 8) as u8,
        b as u8,
    ];
    by[6] = (by[6] & 0x0f) | 0x40; // version 4
    by[8] = (by[8] & 0x3f) | 0x80; // variant
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        by[0], by[1], by[2], by[3], by[4], by[5], by[6], by[7], by[8], by[9], by[10], by[11],
        by[12], by[13], by[14], by[15]
    )
}

fn push_json_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
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
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Parse a comma/newline-separated list of `name:type` fields (the body of an
/// object, without surrounding braces).
fn parse_object_body(s: &str, budget: &mut usize, depth: usize) -> Result<Vec<Field>, String> {
    if depth > MAX_DEPTH {
        return Err(format!("schema nests deeper than {MAX_DEPTH} levels"));
    }
    let mut fields = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        // skip separators (whitespace or commas)
        while i < bytes.len() {
            let c = bytes[i] as char;
            if c.is_whitespace() || c == ',' {
                i += 1;
            } else {
                break;
            }
        }
        if i >= bytes.len() {
            break;
        }
        // read name up to ':'
        let name_start = i;
        while i < bytes.len() && bytes[i] != b':' {
            i += 1;
        }
        if i >= bytes.len() {
            return Err(format!(
                "field '{}' is missing a ':type' (use name:type)",
                s[name_start..].trim()
            ));
        }
        let name = s[name_start..i].trim().to_string();
        if name.is_empty() {
            return Err("a field name is empty".into());
        }
        i += 1; // consume ':'
        while i < bytes.len() && (bytes[i] as char).is_whitespace() {
            i += 1;
        }
        // parse the type spec, which may consume a balanced {...}
        let (ty_str, consumed, is_object_body, obj_body) = read_type_spec(&s[i..])?;
        i += consumed;
        *budget = budget
            .checked_sub(1)
            .ok_or_else(|| format!("schema has more than {MAX_FIELDS} fields"))?;
        let (ty, array) = if is_object_body {
            // an array suffix may follow the closing brace: {...}[3]
            let (arr, used) = parse_array_suffix(&s[i..]);
            i += used;
            (Ty::Object(parse_object_body(&obj_body, budget, depth + 1)?), arr)
        } else {
            let (base, arr) = split_array_suffix(&ty_str)?;
            (parse_type(&base)?, arr)
        };
        fields.push(Field { name, ty, array });
    }
    Ok(fields)
}

/// Read a type spec starting at `s`. Returns (token, bytes_consumed, is_object,
/// object_body). For an object it consumes the balanced `{...}`; for a scalar
/// it reads up to the next top-level comma, consuming balanced `(...)`/`[...]`.
fn read_type_spec(s: &str) -> Result<(String, usize, bool, String), String> {
    let bytes = s.as_bytes();
    if bytes.first() == Some(&b'{') {
        let mut depth = 0i32;
        let mut i = 0usize;
        while i < bytes.len() {
            match bytes[i] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        let body = s[1..i].to_string();
                        return Ok((String::new(), i + 1, true, body));
                    }
                }
                _ => {}
            }
            i += 1;
        }
        return Err("unbalanced '{' in nested object".into());
    }
    let mut i = 0usize;
    let mut paren = 0i32;
    let mut brack = 0i32;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => paren += 1,
            b')' => paren -= 1,
            b'[' => brack += 1,
            b']' => brack -= 1,
            b',' | b'\n' | b'\r' if paren == 0 && brack == 0 => break,
            _ => {}
        }
        i += 1;
    }
    Ok((s[..i].trim().to_string(), i, false, String::new()))
}

/// Parse a leading `[n]` array suffix (after optional whitespace). Returns
/// (Some(n) | None, bytes_consumed).
fn parse_array_suffix(s: &str) -> (Option<u32>, usize) {
    let lead = s.len() - s.trim_start().len();
    let rest = s.trim_start();
    if rest.as_bytes().first() != Some(&b'[') {
        return (None, 0);
    }
    if let Some(end) = rest.find(']') {
        if let Ok(n) = rest[1..end].trim().parse::<u32>() {
            return (Some(n.min(MAX_ARRAY)), lead + end + 1);
        }
    }
    (None, 0)
}

/// Split a scalar type token like `string[3]` into ("string", Some(3)).
fn split_array_suffix(token: &str) -> Result<(String, Option<u32>), String> {
    if token.ends_with(']') {
        if let Some(open) = token.rfind('[') {
            let n: u32 = token[open + 1..token.len() - 1]
                .trim()
                .parse()
                .map_err(|_| format!("invalid array count in '{token}'"))?;
            return Ok((token[..open].trim().to_string(), Some(n.min(MAX_ARRAY))));
        }
    }
    Ok((token.to_string(), None))
}

fn parse_type(token: &str) -> Result<Ty, String> {
    let lower = token.trim().to_ascii_lowercase();
    if let Some(rest) = strip_call(&lower, "enum") {
        let opts: Vec<String> = rest
            .split('|')
            .map(|o| o.trim().to_string())
            .filter(|o| !o.is_empty())
            .collect();
        if opts.is_empty() {
            return Err("enum(...) needs at least one option, e.g. enum(a|b|c)".into());
        }
        return Ok(Ty::Enum(opts));
    }
    if let Some(rest) = strip_call(&lower, "integer").or_else(|| strip_call(&lower, "int")) {
        let (lo, hi) = parse_range_i(rest)?;
        return Ok(Ty::IntRange(lo, hi));
    }
    if let Some(rest) = strip_call(&lower, "float")
        .or_else(|| strip_call(&lower, "number"))
        .or_else(|| strip_call(&lower, "double"))
    {
        let (lo, hi) = parse_range_f(rest)?;
        return Ok(Ty::FloatRange(lo, hi));
    }
    if let Some(rest) = strip_call(&lower, "words") {
        let n: u32 = rest
            .trim()
            .parse()
            .map_err(|_| "words(n) needs a number, e.g. words(5)".to_string())?;
        return Ok(Ty::Words(n.max(1)));
    }
    let ty = match lower.as_str() {
        "string" | "str" | "text" => Ty::StringRand,
        "int" | "integer" => Ty::IntDefault,
        "float" | "number" | "double" => Ty::FloatDefault,
        "bool" | "boolean" => Ty::Bool,
        "uuid" | "guid" => Ty::Uuid,
        "first_name" | "firstname" => Ty::FirstName,
        "last_name" | "lastname" => Ty::LastName,
        "name" | "full_name" | "fullname" => Ty::Name,
        "username" | "handle" => Ty::Username,
        "email" => Ty::Email,
        "phone" | "tel" => Ty::Phone,
        "date" => Ty::Date,
        "datetime" | "timestamp" => Ty::DateTime,
        "url" | "uri" => Ty::Url,
        "domain" => Ty::Domain,
        "word" => Ty::Word,
        "words" => Ty::Words(3),
        "sentence" => Ty::Sentence,
        "paragraph" | "lorem" => Ty::Paragraph,
        "city" => Ty::City,
        "state" => Ty::State,
        "country" => Ty::Country,
        "zip" | "zipcode" | "postcode" => Ty::Zip,
        "street" | "address" => Ty::Street,
        "color" | "colour" => Ty::Color,
        "ipv4" | "ip" => Ty::Ipv4,
        "mac" => Ty::Mac,
        "lat" | "latitude" => Ty::Lat,
        "lng" | "lon" | "longitude" => Ty::Lng,
        "" => return Err("a field is missing its type".into()),
        other => {
            return Err(format!(
                "unknown type '{other}'. Valid: string, int, float, bool, uuid, name, first_name, last_name, username, email, phone, date, datetime, url, domain, word, words, sentence, paragraph, city, state, country, zip, street, color, ipv4, mac, lat, lng, enum(a|b|c), int(lo..hi), float(lo..hi)"
            ))
        }
    };
    Ok(ty)
}

/// If `s` looks like `name(inner)`, return `inner`.
fn strip_call<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix(name) {
        let rest = rest.trim_start();
        if let Some(inner) = rest.strip_prefix('(') {
            if let Some(close) = inner.rfind(')') {
                return Some(inner[..close].trim());
            }
        }
    }
    None
}

fn parse_range_i(s: &str) -> Result<(i64, i64), String> {
    let parts: Vec<&str> = s.split("..").collect();
    if parts.len() != 2 {
        return Err("int(lo..hi) needs two bounds, e.g. int(1..100)".into());
    }
    let lo: i64 = parts[0]
        .trim()
        .parse()
        .map_err(|_| "int range lower bound is not an integer".to_string())?;
    let hi: i64 = parts[1]
        .trim()
        .parse()
        .map_err(|_| "int range upper bound is not an integer".to_string())?;
    Ok((lo, hi))
}

fn parse_range_f(s: &str) -> Result<(f64, f64), String> {
    let parts: Vec<&str> = s.split("..").collect();
    if parts.len() != 2 {
        return Err("float(lo..hi) needs two bounds, e.g. float(0..1)".into());
    }
    let lo: f64 = parts[0]
        .trim()
        .parse()
        .map_err(|_| "float range lower bound is not a number".to_string())?;
    let hi: f64 = parts[1]
        .trim()
        .parse()
        .map_err(|_| "float range upper bound is not a number".to_string())?;
    Ok((lo, hi))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_json(s: &str) -> jsonlite::Value {
        jsonlite::parse(s).unwrap_or_else(|e| panic!("invalid JSON: {e}\n{s}"))
    }

    #[test]
    fn single_object_basic() {
        let out = generate("name:name, age:int(18..65), active:bool", 1, 42, false).unwrap();
        let v = parse_json(&out);
        assert!(v.is_object());
        assert!(v.get("name").unwrap().is_string());
        let age = v.get("age").unwrap().as_i64().unwrap();
        assert!((18..=65).contains(&age));
        assert!(v.get("active").unwrap().is_bool());
    }

    #[test]
    fn array_top_level_count() {
        let out = generate("id:uuid", 5, 7, false).unwrap();
        let v = parse_json(&out);
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 5);
        let id = arr[0].get("id").unwrap().as_str().unwrap();
        assert_eq!(id.len(), 36);
        assert_eq!(id.as_bytes()[14], b'4'); // version nibble
    }

    #[test]
    fn nested_object_and_array_field() {
        let out = generate("user:{first:first_name, last:last_name}, tags:word[3]", 1, 1, false)
            .unwrap();
        let v = parse_json(&out);
        assert!(v.get("user").unwrap().is_object());
        assert!(v.get("user").unwrap().get("first").unwrap().is_string());
        let tags = v.get("tags").unwrap().as_array().unwrap();
        assert_eq!(tags.len(), 3);
    }

    #[test]
    fn object_array_suffix() {
        let out = generate("items:{n:int}[2]", 1, 9, false).unwrap();
        let v = parse_json(&out);
        let items = v.get("items").unwrap().as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert!(items[0].get("n").unwrap().as_i64().is_some());
    }

    #[test]
    fn enum_values_are_from_set() {
        let out = generate("role:enum(admin|user|guest)", 20, 3, false).unwrap();
        let v = parse_json(&out);
        for row in v.as_array().unwrap() {
            let r = row.get("role").unwrap().as_str().unwrap();
            assert!(["admin", "user", "guest"].contains(&r), "got {r}");
        }
    }

    #[test]
    fn deterministic_with_seed() {
        let a = generate("name:name, email:email", 10, 12345, false).unwrap();
        let b = generate("name:name, email:email", 10, 12345, false).unwrap();
        assert_eq!(a, b);
        let c = generate("name:name, email:email", 10, 999, false).unwrap();
        assert_ne!(a, c);
    }

    #[test]
    fn pretty_is_multiline() {
        let out = generate("name:name", 1, 5, true).unwrap();
        assert!(out.contains('\n'));
        let _ = parse_json(&out);
    }

    #[test]
    fn newline_separated_fields() {
        let out = generate("name:name\nemail:email\nage:int", 1, 4, false).unwrap();
        let v = parse_json(&out);
        assert!(v.get("name").unwrap().is_string());
        assert!(v.get("email").unwrap().is_string());
        assert!(v.get("age").unwrap().as_i64().is_some());
    }

    #[test]
    fn empty_schema_errors() {
        assert!(generate("", 1, 1, false).is_err());
        assert!(generate("   ", 1, 1, false).is_err());
    }

    #[test]
    fn missing_type_errors() {
        assert!(generate("name", 1, 1, false).is_err());
    }

    #[test]
    fn unknown_type_errors() {
        let e = generate("x:frobnicate", 1, 1, false).unwrap_err();
        assert!(e.contains("unknown type"), "got {e}");
    }

    #[test]
    fn count_bounds() {
        assert!(generate("a:int", 0, 1, false).is_err());
        assert!(generate("a:int", MAX_COUNT + 1, 1, false).is_err());
    }

    #[test]
    fn float_range_in_bounds() {
        let out = generate("price:float(10..20)", 30, 8, false).unwrap();
        let v = parse_json(&out);
        for row in v.as_array().unwrap() {
            let p = row.get("price").unwrap().as_f64().unwrap();
            assert!((10.0..=20.0).contains(&p), "got {p}");
        }
    }

    #[test]
    fn string_escaping_valid_json() {
        let schema = "a:sentence, b:url, c:color, d:phone, e:email, f:mac, g:ipv4, h:datetime";
        let out = generate(schema, 50, 2, false).unwrap();
        let _ = parse_json(&out);
    }
}

// A tiny JSON parser used ONLY in unit tests so core stays dependency-free in
// the shipped wasm. Behind cfg(test) it can use std freely.
#[cfg(test)]
mod jsonlite {
    //! Minimal recursive-descent JSON parser — enough to validate the
    //! generator's output in tests without a runtime dependency.
    use std::collections::BTreeMap;

    #[derive(Debug, Clone)]
    pub enum Value {
        Null,
        Bool(bool),
        Num(f64),
        Str(String),
        Arr(Vec<Value>),
        Obj(BTreeMap<String, Value>),
    }

    impl Value {
        pub fn is_object(&self) -> bool {
            matches!(self, Value::Obj(_))
        }
        pub fn is_string(&self) -> bool {
            matches!(self, Value::Str(_))
        }
        pub fn is_bool(&self) -> bool {
            matches!(self, Value::Bool(_))
        }
        pub fn get(&self, k: &str) -> Option<&Value> {
            match self {
                Value::Obj(m) => m.get(k),
                _ => None,
            }
        }
        pub fn as_array(&self) -> Option<&Vec<Value>> {
            match self {
                Value::Arr(a) => Some(a),
                _ => None,
            }
        }
        pub fn as_str(&self) -> Option<&str> {
            match self {
                Value::Str(s) => Some(s),
                _ => None,
            }
        }
        pub fn as_f64(&self) -> Option<f64> {
            match self {
                Value::Num(n) => Some(*n),
                _ => None,
            }
        }
        pub fn as_i64(&self) -> Option<i64> {
            match self {
                Value::Num(n) => Some(*n as i64),
                _ => None,
            }
        }
    }

    pub fn parse(s: &str) -> Result<Value, String> {
        let b = s.as_bytes();
        let mut i = 0;
        let v = parse_value(b, &mut i)?;
        skip_ws(b, &mut i);
        if i != b.len() {
            return Err(format!("trailing data at {i}"));
        }
        Ok(v)
    }

    fn skip_ws(b: &[u8], i: &mut usize) {
        while *i < b.len() && (b[*i] as char).is_whitespace() {
            *i += 1;
        }
    }

    fn parse_value(b: &[u8], i: &mut usize) -> Result<Value, String> {
        skip_ws(b, i);
        if *i >= b.len() {
            return Err("unexpected end".into());
        }
        match b[*i] {
            b'{' => parse_obj(b, i),
            b'[' => parse_arr(b, i),
            b'"' => Ok(Value::Str(parse_str(b, i)?)),
            b't' => {
                expect(b, i, "true")?;
                Ok(Value::Bool(true))
            }
            b'f' => {
                expect(b, i, "false")?;
                Ok(Value::Bool(false))
            }
            b'n' => {
                expect(b, i, "null")?;
                Ok(Value::Null)
            }
            _ => parse_num(b, i),
        }
    }

    fn expect(b: &[u8], i: &mut usize, lit: &str) -> Result<(), String> {
        if b[*i..].starts_with(lit.as_bytes()) {
            *i += lit.len();
            Ok(())
        } else {
            Err(format!("expected {lit}"))
        }
    }

    fn parse_num(b: &[u8], i: &mut usize) -> Result<Value, String> {
        let start = *i;
        while *i < b.len() && matches!(b[*i], b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E') {
            *i += 1;
        }
        std::str::from_utf8(&b[start..*i])
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .map(Value::Num)
            .ok_or_else(|| "bad number".into())
    }

    fn parse_str(b: &[u8], i: &mut usize) -> Result<String, String> {
        *i += 1; // opening quote
        let mut out = String::new();
        while *i < b.len() {
            match b[*i] {
                b'"' => {
                    *i += 1;
                    return Ok(out);
                }
                b'\\' => {
                    *i += 1;
                    match b[*i] {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hex = std::str::from_utf8(&b[*i + 1..*i + 5]).unwrap();
                            let cp = u32::from_str_radix(hex, 16).unwrap();
                            out.push(char::from_u32(cp).unwrap_or('?'));
                            *i += 4;
                        }
                        other => out.push(other as char),
                    }
                    *i += 1;
                }
                c => {
                    out.push(c as char);
                    *i += 1;
                }
            }
        }
        Err("unterminated string".into())
    }

    fn parse_arr(b: &[u8], i: &mut usize) -> Result<Value, String> {
        *i += 1;
        let mut arr = Vec::new();
        skip_ws(b, i);
        if b[*i] == b']' {
            *i += 1;
            return Ok(Value::Arr(arr));
        }
        loop {
            arr.push(parse_value(b, i)?);
            skip_ws(b, i);
            match b[*i] {
                b',' => *i += 1,
                b']' => {
                    *i += 1;
                    return Ok(Value::Arr(arr));
                }
                _ => return Err("expected , or ]".into()),
            }
        }
    }

    fn parse_obj(b: &[u8], i: &mut usize) -> Result<Value, String> {
        *i += 1;
        let mut m = BTreeMap::new();
        skip_ws(b, i);
        if b[*i] == b'}' {
            *i += 1;
            return Ok(Value::Obj(m));
        }
        loop {
            skip_ws(b, i);
            let k = parse_str(b, i)?;
            skip_ws(b, i);
            if b[*i] != b':' {
                return Err("expected :".into());
            }
            *i += 1;
            let v = parse_value(b, i)?;
            m.insert(k, v);
            skip_ws(b, i);
            match b[*i] {
                b',' => *i += 1,
                b'}' => {
                    *i += 1;
                    return Ok(Value::Obj(m));
                }
                _ => return Err("expected , or }".into()),
            }
        }
    }
}
