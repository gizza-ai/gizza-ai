//! json-schema-faker core — generate N rows of realistic fake data that CONFORM
//! to a JSON Schema, respecting string formats (email, uuid, date, date-time,
//! uri, ipv4), enum/const, and length/range/item bounds.
//!
//! Pure Rust (serde_json only) → runs on ALL backends including the chat Service
//! Worker. Deterministic given a non-zero `seed`; the chat/web surfaces derive a
//! varying seed from the clock when `seed == 0` so repeated calls differ.
//!
//! In-model JSON Schema subset: object roots with `properties`; `required`;
//! types string/integer/number/boolean/array/object; `enum`/`const`; string
//! `format` email/uuid/date/date-time/uri/ipv4; `minLength`/`maxLength`;
//! `minimum`/`maximum`; `minItems`/`maxItems`; `additionalProperties` (never
//! invents extra keys). Unsupported ASSERTIONS ($ref, oneOf/anyOf/allOf/not,
//! patternProperties, dependencies, if/then/else, pattern) are hard errors — we
//! refuse rather than silently emit non-conforming data. Unknown string formats
//! fall back to a plain string (format is an annotation, still conforms).

use serde_json::{Map, Number, Value};

/// Maximum rows generated per call.
pub const MAX_COUNT: u32 = 1000;

/// Guard against pathological schema recursion depth.
const MAX_DEPTH: usize = 32;

// ---- deterministic data pools (synthetic only, never real people) ----------
const FIRST_NAMES: &[&str] = &[
    "ava", "liam", "mia", "noah", "emma", "oliver", "sophia", "lucas", "isabella", "mason",
    "amelia", "ethan", "harper", "james", "evelyn", "aria", "leo", "nora", "owen", "zoe",
];
const LAST_NAMES: &[&str] = &[
    "smith", "jones", "brown", "taylor", "wilson", "davies", "evans", "thomas", "roberts",
    "walker", "wright", "green", "hall", "clark", "patel", "khan", "lee", "young", "king", "scott",
];
const DOMAINS: &[&str] = &[
    "example.com",
    "test.org",
    "sample.net",
    "demo.io",
    "mock.dev",
];
const WORDS: &[&str] = &[
    "lorem",
    "ipsum",
    "dolor",
    "sit",
    "amet",
    "consectetur",
    "adipiscing",
    "elit",
    "sed",
    "do",
    "eiusmod",
    "tempor",
    "incididunt",
    "labore",
    "magna",
    "aliqua",
    "enim",
    "minim",
    "veniam",
    "quis",
    "nostrud",
    "ullamco",
    "laboris",
    "nisi",
    "aliquip",
    "commodo",
    "consequat",
    "duis",
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
    /// Inclusive integer in [lo, hi] (lo == hi → lo; lo > hi → lo).
    fn range_i(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            return lo;
        }
        let span = (hi - lo) as u64 + 1;
        lo + self.below(span) as i64
    }
    fn range_f(&mut self, lo: f64, hi: f64) -> f64 {
        if hi <= lo {
            return lo;
        }
        let u = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        lo + u * (hi - lo)
    }
    fn bool(&mut self) -> bool {
        self.next_u64() & 1 == 0
    }
}

/// Public entry: produce `count` rows conforming to `schema_src`. `output` is
/// one of `json` (a JSON array, `pretty` toggles indentation), `jsonl` (one
/// compact JSON value per line), or `csv` (requires object rows).
pub fn generate(
    schema_src: &str,
    count: u32,
    seed: u64,
    pretty: bool,
    output: &str,
) -> Result<String, String> {
    if count == 0 {
        return Err("count must be at least 1".into());
    }
    if count > MAX_COUNT {
        return Err(format!("count {count} exceeds the maximum of {MAX_COUNT}"));
    }
    let output = output.trim().to_ascii_lowercase();
    if !matches!(output.as_str(), "json" | "jsonl" | "csv") {
        return Err(format!(
            "invalid output {output:?}: expected \"json\", \"jsonl\", or \"csv\""
        ));
    }

    let schema_val: Value = serde_json::from_str(schema_src.trim())
        .map_err(|e| format!("schema is not valid JSON: {e}"))?;
    let schema = schema_val.as_object().ok_or_else(|| {
        "JSON Schema must be an object (e.g. {\"type\":\"object\", ...})".to_string()
    })?;

    check_schema(schema)?;

    let mut rng = Rng::new(if seed == 0 {
        0xC0FF_EE12_3456_789A
    } else {
        seed
    });
    let mut rows: Vec<Value> = Vec::with_capacity(count as usize);
    for _ in 0..count {
        rows.push(gen_value(schema, &mut rng, 0)?);
    }

    match output.as_str() {
        "jsonl" => Ok(rows
            .iter()
            .map(|r| serde_json::to_string(r).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n")),
        "csv" => to_csv(&rows),
        _ => {
            let arr = Value::Array(rows);
            if pretty {
                serde_json::to_string_pretty(&arr)
            } else {
                serde_json::to_string(&arr)
            }
            .map_err(|e| format!("failed to serialize JSON: {e}"))
        }
    }
}

/// Keywords this faker cannot honor. Present in a schema node → hard error
/// (they are assertions; silently ignoring them would emit non-conforming data).
const UNSUPPORTED: &[&str] = &[
    "$ref",
    "$dynamicRef",
    "oneOf",
    "anyOf",
    "allOf",
    "not",
    "if",
    "then",
    "else",
    "patternProperties",
    "dependencies",
    "dependentSchemas",
    "dependentRequired",
    "pattern",
];

/// Schema-aware walk: reject unsupported keywords at every SCHEMA position, but
/// never treat a user's property NAME (a key inside `properties`) as a keyword.
fn check_schema(node: &Map<String, Value>) -> Result<(), String> {
    for &kw in UNSUPPORTED {
        if node.contains_key(kw) {
            return Err(format!(
                "unsupported JSON Schema construct {kw:?}: this faker handles object/array/scalar \
                 types, enum, const, string formats (email, uuid, date, date-time, uri, ipv4), and \
                 length/range/item bounds. It does not resolve {kw:?} — remove it or expand the \
                 schema into the supported subset."
            ));
        }
    }
    // Recurse only into genuine sub-schema positions.
    if let Some(Value::Object(props)) = node.get("properties") {
        for sub in props.values() {
            recurse_subschema(sub)?;
        }
    }
    match node.get("items") {
        Some(v @ Value::Object(_)) => recurse_subschema(v)?,
        Some(Value::Array(tuple)) => {
            for sub in tuple {
                recurse_subschema(sub)?;
            }
        }
        _ => {}
    }
    if let Some(Value::Array(tuple)) = node.get("prefixItems") {
        for sub in tuple {
            recurse_subschema(sub)?;
        }
    }
    if let Some(ap) = node.get("additionalProperties") {
        recurse_subschema(ap)?;
    }
    Ok(())
}

fn recurse_subschema(v: &Value) -> Result<(), String> {
    if let Value::Object(o) = v {
        check_schema(o)
    } else {
        Ok(()) // boolean schema (true/false) — nothing to check
    }
}

/// Generate one value conforming to `schema`.
fn gen_value(schema: &Map<String, Value>, rng: &mut Rng, depth: usize) -> Result<Value, String> {
    if depth > MAX_DEPTH {
        return Err(format!("schema nesting exceeds {MAX_DEPTH} levels"));
    }
    // const wins outright.
    if let Some(c) = schema.get("const") {
        return Ok(c.clone());
    }
    // enum → pick one of the allowed values.
    if let Some(Value::Array(choices)) = schema.get("enum") {
        if choices.is_empty() {
            return Err("enum must list at least one value".into());
        }
        return Ok(choices[rng.below(choices.len() as u64) as usize].clone());
    }

    let ty = resolve_type(schema);
    match ty {
        "object" => gen_object(schema, rng, depth),
        "array" => gen_array(schema, rng, depth),
        "boolean" => Ok(Value::Bool(rng.bool())),
        "integer" => Ok(gen_integer(schema, rng)),
        "number" => Ok(gen_number(schema, rng)),
        "null" => Ok(Value::Null),
        "string" => Ok(Value::String(gen_string(schema, rng)?)),
        other => Err(format!(
            "unsupported type {other:?}: expected string, integer, number, boolean, array, object, \
             or null"
        )),
    }
}

/// Decide the effective type: explicit `type` (first non-null entry if it is an
/// array), else inferred from structural keywords, else `string`.
fn resolve_type(schema: &Map<String, Value>) -> &'static str {
    let name = match schema.get("type") {
        Some(Value::String(s)) => Some(s.as_str()),
        Some(Value::Array(list)) => list
            .iter()
            .filter_map(|v| v.as_str())
            .find(|s| *s != "null")
            .or_else(|| list.iter().filter_map(|v| v.as_str()).next()),
        _ => None,
    };
    match name {
        Some("object") => "object",
        Some("array") => "array",
        Some("boolean") => "boolean",
        Some("integer") => "integer",
        Some("number") => "number",
        Some("null") => "null",
        Some("string") => "string",
        Some(_) => "unknown",
        None => {
            if schema.contains_key("properties") {
                "object"
            } else if schema.contains_key("items") || schema.contains_key("prefixItems") {
                "array"
            } else {
                "string"
            }
        }
    }
}

fn gen_object(schema: &Map<String, Value>, rng: &mut Rng, depth: usize) -> Result<Value, String> {
    let mut out = Map::new();
    // Every DEFINED property is generated → `required` is always satisfied.
    if let Some(Value::Object(props)) = schema.get("properties") {
        for (name, sub) in props {
            let value = match sub {
                Value::Object(o) => gen_value(o, rng, depth + 1)?,
                Value::Bool(_) => Value::String(gen_words(rng, 1, 2)),
                _ => Value::Null,
            };
            out.insert(name.clone(), value);
        }
    }
    Ok(Value::Object(out))
}

fn gen_array(schema: &Map<String, Value>, rng: &mut Rng, depth: usize) -> Result<Value, String> {
    // Tuple form: items / prefixItems as an array → one value per entry.
    if let Some(Value::Array(tuple)) = schema.get("items").or_else(|| schema.get("prefixItems")) {
        let mut items = Vec::with_capacity(tuple.len());
        for sub in tuple {
            items.push(match sub {
                Value::Object(o) => gen_value(o, rng, depth + 1)?,
                _ => Value::Null,
            });
        }
        return Ok(Value::Array(items));
    }

    let min = schema.get("minItems").and_then(Value::as_u64);
    let max = schema.get("maxItems").and_then(Value::as_u64);
    let mut lo = min.unwrap_or(1);
    let mut hi = max.unwrap_or(lo.max(1) + 2);
    if let Some(m) = max {
        lo = lo.min(m); // never generate more than maxItems even without minItems
    }
    if hi < lo {
        hi = lo;
    }
    let n = rng.range_i(lo as i64, hi as i64).max(0) as usize;

    let item_schema = match schema.get("items") {
        Some(Value::Object(o)) => Some(o),
        _ => None,
    };
    let mut items = Vec::with_capacity(n);
    for _ in 0..n {
        let v = match item_schema {
            Some(o) => gen_value(o, rng, depth + 1)?,
            None => Value::String(gen_words(rng, 1, 2)),
        };
        items.push(v);
    }
    Ok(Value::Array(items))
}

fn gen_integer(schema: &Map<String, Value>, rng: &mut Rng) -> Value {
    let lo = schema
        .get("minimum")
        .and_then(Value::as_f64)
        .map(|v| v.ceil() as i64);
    let hi = schema
        .get("maximum")
        .and_then(Value::as_f64)
        .map(|v| v.floor() as i64);
    let (lo, hi) = match (lo, hi) {
        (Some(a), Some(b)) => (a, b.max(a)),
        (Some(a), None) => (a, a + 1000),
        (None, Some(b)) => (b - 1000, b),
        (None, None) => (0, 1000),
    };
    Value::from(rng.range_i(lo, hi))
}

fn gen_number(schema: &Map<String, Value>, rng: &mut Rng) -> Value {
    let lo = schema.get("minimum").and_then(Value::as_f64);
    let hi = schema.get("maximum").and_then(Value::as_f64);
    let (lo, hi) = match (lo, hi) {
        (Some(a), Some(b)) => (a, b.max(a)),
        (Some(a), None) => (a, a + 1000.0),
        (None, Some(b)) => (b - 1000.0, b),
        (None, None) => (0.0, 1000.0),
    };
    let v = (rng.range_f(lo, hi) * 100.0).round() / 100.0;
    Number::from_f64(v)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

fn gen_string(schema: &Map<String, Value>, rng: &mut Rng) -> Result<String, String> {
    if let Some(Value::String(fmt)) = schema.get("format") {
        match fmt.as_str() {
            "email" => {
                return Ok(format!(
                    "{}.{}@{}",
                    rng.pick(FIRST_NAMES),
                    rng.pick(LAST_NAMES),
                    rng.pick(DOMAINS)
                ))
            }
            "uuid" => return Ok(uuid_v4(rng)),
            "date" => {
                return Ok(format!(
                    "{:04}-{:02}-{:02}",
                    rng.range_i(1970, 2024),
                    rng.range_i(1, 12),
                    rng.range_i(1, 28)
                ))
            }
            "date-time" => {
                return Ok(format!(
                    "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                    rng.range_i(1970, 2024),
                    rng.range_i(1, 12),
                    rng.range_i(1, 28),
                    rng.range_i(0, 23),
                    rng.range_i(0, 59),
                    rng.range_i(0, 59)
                ))
            }
            "uri" => return Ok(format!("https://{}/{}", rng.pick(DOMAINS), rng.pick(WORDS))),
            "ipv4" => {
                return Ok(format!(
                    "{}.{}.{}.{}",
                    rng.below(256),
                    rng.below(256),
                    rng.below(256),
                    rng.below(256)
                ))
            }
            // Unknown format is an annotation, not an assertion → a plain string
            // still conforms. Fall through to length-bounded generation.
            _ => {}
        }
    }

    let min = schema.get("minLength").and_then(Value::as_u64);
    let max = schema.get("maxLength").and_then(Value::as_u64);
    if let (Some(a), Some(b)) = (min, max) {
        if a > b {
            return Err(format!("minLength {a} exceeds maxLength {b}"));
        }
    }
    if min.is_none() && max.is_none() {
        return Ok(gen_words(rng, 1, 2));
    }
    let lo = min.unwrap_or(0);
    let hi = max.unwrap_or(lo.max(1) + 8);
    let target = rng.range_i(lo as i64, hi as i64).max(0) as usize;
    // Build a word-ish string, then take exactly `target` chars.
    let mut s = String::new();
    while s.chars().count() < target {
        s.push_str(rng.pick(WORDS));
    }
    Ok(s.chars().take(target).collect())
}

/// `min`..=`max` words joined by '-'.
fn gen_words(rng: &mut Rng, min: usize, max: usize) -> String {
    let n = min.max(1) + rng.below((max.max(min) - min + 1) as u64) as usize;
    (0..n)
        .map(|_| rng.pick(WORDS))
        .collect::<Vec<_>>()
        .join("-")
}

fn uuid_v4(rng: &mut Rng) -> String {
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
        by[0], by[1], by[2], by[3], by[4], by[5], by[6], by[7],
        by[8], by[9], by[10], by[11], by[12], by[13], by[14], by[15]
    )
}

fn to_csv(rows: &[Value]) -> Result<String, String> {
    // Ordered union of keys across all rows.
    let mut headers: Vec<String> = Vec::new();
    for row in rows {
        let obj = row.as_object().ok_or_else(|| {
            "CSV output requires the schema to produce objects (root type \"object\"); use json or \
             jsonl for scalar or array roots"
                .to_string()
        })?;
        for k in obj.keys() {
            if !headers.iter().any(|h| h == k) {
                headers.push(k.clone());
            }
        }
    }
    let mut lines = Vec::with_capacity(rows.len() + 1);
    lines.push(
        headers
            .iter()
            .map(|h| csv_escape(h))
            .collect::<Vec<_>>()
            .join(","),
    );
    for row in rows {
        let obj = row.as_object().unwrap();
        let cells: Vec<String> = headers
            .iter()
            .map(|h| csv_escape(&cell_to_string(obj.get(h))))
            .collect();
        lines.push(cells.join(","));
    }
    Ok(lines.join("\n"))
}

fn cell_to_string(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => serde_json::to_string(other).unwrap_or_default(),
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn gen(schema: &str, count: u32, seed: u64, pretty: bool, output: &str) -> String {
        generate(schema, count, seed, pretty, output).expect("generate ok")
    }

    #[test]
    fn deterministic_for_seed() {
        let s = r#"{"type":"object","properties":{"id":{"type":"integer"},"email":{"type":"string","format":"email"}}}"#;
        let a = gen(s, 3, 42, true, "json");
        let b = gen(s, 3, 42, true, "json");
        assert_eq!(a, b, "same seed → same output");
        let c = gen(s, 3, 43, true, "json");
        assert_ne!(a, c, "different seed → different output");
    }

    #[test]
    fn conforms_types_and_formats() {
        let s = r#"{
            "type":"object",
            "required":["id","email","when","tags"],
            "properties":{
                "id":{"type":"integer","minimum":1,"maximum":100},
                "score":{"type":"number","minimum":0,"maximum":1},
                "active":{"type":"boolean"},
                "email":{"type":"string","format":"email"},
                "uid":{"type":"string","format":"uuid"},
                "when":{"type":"string","format":"date-time"},
                "day":{"type":"string","format":"date"},
                "site":{"type":"string","format":"uri"},
                "ip":{"type":"string","format":"ipv4"},
                "tags":{"type":"array","items":{"type":"string"},"minItems":2,"maxItems":2}
            }
        }"#;
        let out = gen(s, 5, 7, true, "json");
        let rows: Value = serde_json::from_str(&out).unwrap();
        let rows = rows.as_array().unwrap();
        assert_eq!(rows.len(), 5);
        for row in rows {
            let o = row.as_object().unwrap();
            for k in ["id", "email", "when", "tags"] {
                assert!(o.contains_key(k), "missing {k}");
            }
            let id = o["id"].as_i64().unwrap();
            assert!((1..=100).contains(&id), "id {id} out of range");
            let score = o["score"].as_f64().unwrap();
            assert!((0.0..=1.0).contains(&score), "score {score} out of range");
            assert!(o["active"].is_boolean());
            let email = o["email"].as_str().unwrap();
            assert!(
                email.contains('@') && email.contains('.'),
                "bad email {email}"
            );
            let uid = o["uid"].as_str().unwrap();
            assert_eq!(uid.len(), 36, "uuid length");
            assert_eq!(uid.chars().filter(|&c| c == '-').count(), 4);
            assert!(o["when"].as_str().unwrap().ends_with('Z'));
            assert_eq!(o["day"].as_str().unwrap().len(), 10);
            assert!(o["site"].as_str().unwrap().starts_with("https://"));
            let ip = o["ip"].as_str().unwrap();
            assert_eq!(ip.split('.').count(), 4, "ipv4 shape {ip}");
            let tags = o["tags"].as_array().unwrap();
            assert_eq!(tags.len(), 2, "minItems==maxItems==2");
        }
    }

    #[test]
    fn enum_and_const_respected() {
        let s = r#"{"type":"object","properties":{
            "role":{"enum":["admin","user","guest"]},
            "kind":{"const":"account"}
        }}"#;
        let out = gen(s, 20, 1, false, "json");
        let rows: Value = serde_json::from_str(&out).unwrap();
        for row in rows.as_array().unwrap() {
            let role = row["role"].as_str().unwrap();
            assert!(
                ["admin", "user", "guest"].contains(&role),
                "enum leak {role}"
            );
            assert_eq!(row["kind"].as_str().unwrap(), "account");
        }
    }

    #[test]
    fn string_length_bounds() {
        let s = r#"{"type":"object","properties":{"code":{"type":"string","minLength":8,"maxLength":8}}}"#;
        let out = gen(s, 10, 3, false, "json");
        let rows: Value = serde_json::from_str(&out).unwrap();
        for row in rows.as_array().unwrap() {
            assert_eq!(row["code"].as_str().unwrap().chars().count(), 8);
        }
    }

    #[test]
    fn jsonl_and_csv_output() {
        let s = r#"{"type":"object","properties":{"id":{"type":"integer","minimum":1,"maximum":9},"name":{"type":"string","minLength":3,"maxLength":3}}}"#;
        let jsonl = gen(s, 3, 5, true, "jsonl");
        assert_eq!(jsonl.lines().count(), 3);
        for line in jsonl.lines() {
            let v: Value = serde_json::from_str(line).unwrap();
            assert!(v.is_object());
        }
        let csv = gen(s, 3, 5, true, "csv");
        let mut clines = csv.lines();
        assert_eq!(clines.next().unwrap(), "id,name");
        assert_eq!(csv.lines().count(), 4, "header + 3 rows");
    }

    #[test]
    fn rejects_unsupported_constructs() {
        let bad = [
            r##"{"$ref":"#/x"}"##,
            r#"{"oneOf":[{"type":"string"}]}"#,
            r#"{"anyOf":[{"type":"string"}]}"#,
            r#"{"allOf":[{"type":"string"}]}"#,
            r#"{"type":"string","pattern":"^a+$"}"#,
            r##"{"type":"object","properties":{"x":{"$ref":"#/y"}}}"##,
        ];
        for b in bad {
            assert!(
                generate(b, 1, 1, false, "json").is_err(),
                "should reject: {b}"
            );
        }
    }

    #[test]
    fn property_named_like_keyword_is_ok() {
        // A user field literally named "pattern"/"if" must NOT be mistaken for a keyword.
        let s = r#"{"type":"object","properties":{"pattern":{"type":"string"},"if":{"type":"integer"}}}"#;
        let out = generate(s, 1, 1, false, "json");
        assert!(
            out.is_ok(),
            "property names must not trip the keyword check: {out:?}"
        );
    }

    #[test]
    fn count_cap_and_errors() {
        let s = r#"{"type":"object","properties":{}}"#;
        assert!(generate(s, 1001, 1, false, "json").is_err(), "over cap");
        assert!(generate(s, 0, 1, false, "json").is_err(), "zero count");
        assert!(
            generate(s, 1000, 1, false, "json").is_ok(),
            "cap boundary ok"
        );
        assert!(
            generate("{ not json", 1, 1, false, "json").is_err(),
            "bad json"
        );
        assert!(
            generate("[1,2]", 1, 1, false, "json").is_err(),
            "non-object schema"
        );
        assert!(
            generate(s, 1, 1, false, "yaml").is_err(),
            "bad output format"
        );
    }
}
