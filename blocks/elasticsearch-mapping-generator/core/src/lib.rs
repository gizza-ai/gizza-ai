//! elasticsearch-mapping-generator core — infer an Elasticsearch index mapping
//! from a sample JSON document (or an array of sample documents).
//!
//! Pure Rust (serde_json only), no wafer/wasm-bindgen deps, so the same logic
//! backs the chat block, the CLI and the browser page.
//!
//! The inference follows Elasticsearch's own dynamic field-mapping rules as the
//! baseline — booleans map to `boolean`, whole numbers to `long`, fractional
//! numbers to `float`, objects to `object` + `properties`, `null` and empty
//! arrays add no field at all, and strings become `text` with a `.keyword`
//! sub-field unless date (default on) or numeric (default off) detection claims
//! them first. Everything else is an opt-in refinement: keyword-only or
//! text-only string strategies, narrower/wider numeric widths, `nested` arrays
//! of objects, `ip` and `geo_point` detection, an analyzer for text fields, and
//! an optional `settings` block with shards/replicas.
//!
//! Unlike the dynamic rules — which take an array's type from its FIRST
//! non-null element — every observation of a field is MERGED here: an integer
//! seen alongside a fractional number widens to the float type, and genuinely
//! incompatible observations (say boolean and object) widen to the string
//! strategy rather than picking whichever came first.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

/// Hard cap on nesting depth, so a pathological document can't blow the stack.
pub const MAX_DEPTH: usize = 50;

/// Largest `ignore_above` Elasticsearch accepts for a `keyword` field (the
/// Lucene term-byte limit).
pub const MAX_IGNORE_ABOVE: i64 = 32766;

/// Which document to emit around the inferred `properties`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    /// `{ "mappings": { … } }` — the default; ready for `PUT /index/_mapping`
    /// (inner object) or as the mappings half of a create-index body.
    Mappings,
    /// `{ "settings": { … }, "mappings": { … } }` — a full `PUT /index` body.
    CreateIndex,
    /// `{ "properties": { … } }` — just the field definitions.
    Properties,
}

impl Output {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "mappings" | "mapping" => Ok(Output::Mappings),
            "create-index" | "create_index" | "index" | "settings" => Ok(Output::CreateIndex),
            "properties" | "props" | "fields" => Ok(Output::Properties),
            other => Err(format!(
                "unknown output '{other}' — use mappings, create-index or properties"
            )),
        }
    }
}

/// How JSON strings that are not dates/numbers/ips become fields.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextFields {
    /// `text` with a `keyword` sub-field (Elasticsearch's own default).
    TextKeyword,
    /// `keyword` only — exact matching, aggregations and sorting, no analysis.
    Keyword,
    /// `text` only — full-text search, no exact-match sub-field.
    Text,
}

impl TextFields {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "text_keyword" | "text-keyword" | "both" => Ok(TextFields::TextKeyword),
            "keyword" => Ok(TextFields::Keyword),
            "text" => Ok(TextFields::Text),
            other => Err(format!(
                "unknown text_fields '{other}' — use text_keyword, keyword or text"
            )),
        }
    }

    fn has_text(self) -> bool {
        matches!(self, TextFields::TextKeyword | TextFields::Text)
    }

    fn has_keyword(self) -> bool {
        matches!(self, TextFields::TextKeyword | TextFields::Keyword)
    }
}

/// How an array of objects is mapped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArrayObjects {
    /// `object` — Elasticsearch's default; sub-fields are flattened, so
    /// cross-field matching within one array element is not preserved.
    Object,
    /// `nested` — each array element is indexed as its own hidden document, so
    /// `nested` queries can match two sub-fields on the SAME element.
    Nested,
}

impl ArrayObjects {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "object" => Ok(ArrayObjects::Object),
            "nested" => Ok(ArrayObjects::Nested),
            other => Err(format!(
                "unknown array_objects '{other}' — use object or nested"
            )),
        }
    }
}

fn parse_dynamic(s: &str) -> Result<&'static str, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "true" | "yes" => Ok("true"),
        "false" | "no" => Ok("false"),
        "strict" => Ok("strict"),
        "runtime" => Ok("runtime"),
        other => Err(format!(
            "unknown dynamic '{other}' — use true, false, strict or runtime"
        )),
    }
}

fn parse_int_type(s: &str) -> Result<&'static str, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "long" => Ok("long"),
        "integer" | "int" => Ok("integer"),
        "short" => Ok("short"),
        other => Err(format!(
            "unknown integer_type '{other}' — use long, integer or short"
        )),
    }
}

fn parse_float_type(s: &str) -> Result<&'static str, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "float" => Ok("float"),
        "double" => Ok("double"),
        "half_float" | "half-float" => Ok("half_float"),
        other => Err(format!(
            "unknown float_type '{other}' — use float, double or half_float"
        )),
    }
}

/// Everything the generator can be told, as raw surface strings/numbers so each
/// surface (chat, CLI, page) can hand them over unparsed.
#[derive(Clone, Debug)]
pub struct Options {
    pub output: String,
    pub text_fields: String,
    pub ignore_above: i64,
    pub analyzer: String,
    pub integer_type: String,
    pub float_type: String,
    pub date_detection: bool,
    pub numeric_detection: bool,
    pub detect_ip: bool,
    pub detect_geo_point: bool,
    pub array_objects: String,
    pub dynamic: String,
    pub shards: i64,
    pub replicas: i64,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            output: "mappings".into(),
            text_fields: "text_keyword".into(),
            ignore_above: 256,
            analyzer: String::new(),
            integer_type: "long".into(),
            float_type: "float".into(),
            date_detection: true,
            numeric_detection: false,
            detect_ip: false,
            detect_geo_point: false,
            array_objects: "object".into(),
            dynamic: "true".into(),
            shards: 1,
            replicas: 1,
        }
    }
}

/// Validated options.
struct Config {
    output: Output,
    text_fields: TextFields,
    ignore_above: i64,
    analyzer: String,
    integer_type: &'static str,
    float_type: &'static str,
    date_detection: bool,
    numeric_detection: bool,
    detect_ip: bool,
    detect_geo_point: bool,
    array_objects: ArrayObjects,
    dynamic: &'static str,
    shards: i64,
    replicas: i64,
}

impl Config {
    fn build(o: &Options) -> Result<Self, String> {
        if o.ignore_above < 0 || o.ignore_above > MAX_IGNORE_ABOVE {
            return Err(format!(
                "ignore_above must be between 0 and {MAX_IGNORE_ABOVE} (0 disables it), got {}",
                o.ignore_above
            ));
        }
        if o.shards < 1 || o.shards > 1024 {
            return Err(format!(
                "shards must be between 1 and 1024, got {}",
                o.shards
            ));
        }
        if o.replicas < 0 || o.replicas > 100 {
            return Err(format!(
                "replicas must be between 0 and 100, got {}",
                o.replicas
            ));
        }
        let analyzer = o.analyzer.trim().to_string();
        if analyzer.contains(char::is_whitespace) {
            return Err(format!(
                "analyzer must be a single analyzer name without spaces, got '{analyzer}'"
            ));
        }
        Ok(Config {
            output: Output::parse(&o.output)?,
            text_fields: TextFields::parse(&o.text_fields)?,
            ignore_above: o.ignore_above,
            analyzer,
            integer_type: parse_int_type(&o.integer_type)?,
            float_type: parse_float_type(&o.float_type)?,
            date_detection: o.date_detection,
            numeric_detection: o.numeric_detection,
            detect_ip: o.detect_ip,
            detect_geo_point: o.detect_geo_point,
            array_objects: ArrayObjects::parse(&o.array_objects)?,
            dynamic: parse_dynamic(&o.dynamic)?,
            shards: o.shards,
            replicas: o.replicas,
        })
    }
}

/// The scalar Elasticsearch types the inference can settle on, ordered so that
/// merging two different ones can widen predictably.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Scalar {
    Boolean,
    Integer,
    Float,
    Date,
    /// A `date` that needed a non-default `format` (slash-style dates).
    DateSlash,
    Ip,
    Str,
}

/// The inferred shape of one field, before it is rendered to a mapping.
#[derive(Clone, Debug)]
enum Shape {
    /// Only `null`s / empty arrays were seen — Elasticsearch adds no field.
    Unknown,
    Scalar(Scalar),
    /// `{"lat": …, "lon": …}` when geo-point detection is on.
    GeoPoint,
    Object {
        /// Sorted so two runs of the same input diff cleanly.
        properties: BTreeMap<String, Shape>,
        /// True once the field was seen as an array of objects.
        in_array: bool,
    },
}

impl Shape {
    /// Merge two observations of the same field into one shape.
    fn merge(self, other: Shape) -> Shape {
        match (self, other) {
            (Shape::Unknown, o) => o,
            (s, Shape::Unknown) => s,
            (Shape::GeoPoint, Shape::GeoPoint) => Shape::GeoPoint,
            (
                Shape::Object {
                    properties: mut a,
                    in_array: aa,
                },
                Shape::Object {
                    properties: b,
                    in_array: ba,
                },
            ) => {
                for (k, v) in b {
                    let merged = match a.remove(&k) {
                        Some(prev) => prev.merge(v),
                        None => v,
                    };
                    a.insert(k, merged);
                }
                Shape::Object {
                    properties: a,
                    in_array: aa || ba,
                }
            }
            (Shape::Scalar(a), Shape::Scalar(b)) => Shape::Scalar(merge_scalar(a, b)),
            // object vs geo_point, object vs scalar, geo_point vs scalar: no
            // Elasticsearch type covers both, so widen to the string strategy —
            // the one mapping that will at least accept the raw values.
            _ => Shape::Scalar(Scalar::Str),
        }
    }
}

fn merge_scalar(a: Scalar, b: Scalar) -> Scalar {
    use Scalar::*;
    if a == b {
        return a;
    }
    match (a, b) {
        // Whole numbers seen alongside fractional ones widen to the float type.
        (Integer, Float) | (Float, Integer) => Float,
        // Two date flavours in one field need the explicit format list.
        (Date, DateSlash) | (DateSlash, Date) => DateSlash,
        // Anything else is genuinely incompatible.
        _ => Str,
    }
}

/// Infer the shape of one JSON value.
fn shape_of(v: &Value, cfg: &Config, depth: usize) -> Result<Shape, String> {
    if depth > MAX_DEPTH {
        return Err(format!(
            "sample nests deeper than {MAX_DEPTH} levels — flatten it or map the deep part by hand"
        ));
    }
    Ok(match v {
        Value::Null => Shape::Unknown,
        Value::Bool(_) => Shape::Scalar(Scalar::Boolean),
        Value::Number(n) => {
            if n.is_f64() && n.as_f64().map(|f| f.fract() != 0.0).unwrap_or(false) {
                Shape::Scalar(Scalar::Float)
            } else if n.is_i64() || n.is_u64() {
                Shape::Scalar(Scalar::Integer)
            } else {
                // A float that happens to be whole (1.0): JSON has one number
                // type, so treat it as fractional — narrowing to an integer
                // type would reject 1.5 on the next document.
                Shape::Scalar(Scalar::Float)
            }
        }
        Value::String(s) => Shape::Scalar(string_scalar(s, cfg)),
        Value::Array(items) => {
            // Elasticsearch flattens arrays: the field's type is its element
            // type. Merge every element rather than trusting the first.
            let mut shape = Shape::Unknown;
            for item in items {
                shape = shape.merge(shape_of(item, cfg, depth + 1)?);
            }
            if let Shape::Object {
                properties,
                in_array: _,
            } = shape
            {
                Shape::Object {
                    properties,
                    in_array: true,
                }
            } else {
                shape
            }
        }
        Value::Object(map) => {
            if cfg.detect_geo_point && is_geo_point(map) {
                return Ok(Shape::GeoPoint);
            }
            let mut properties = BTreeMap::new();
            for (k, val) in map {
                properties.insert(k.clone(), shape_of(val, cfg, depth + 1)?);
            }
            Shape::Object {
                properties,
                in_array: false,
            }
        }
    })
}

/// `{"lat": <number>, "lon": <number>}` — the object form Elasticsearch accepts
/// for `geo_point`. Exactly those two keys, nothing else.
fn is_geo_point(map: &Map<String, Value>) -> bool {
    map.len() == 2
        && map.get("lat").map(Value::is_number).unwrap_or(false)
        && map.get("lon").map(Value::is_number).unwrap_or(false)
}

/// Classify a JSON string, in Elasticsearch's own detection order: date first,
/// then numeric, then (as an extension) ip, then the string strategy.
fn string_scalar(s: &str, cfg: &Config) -> Scalar {
    let t = s.trim();
    if cfg.date_detection {
        if is_iso_date(t) {
            return Scalar::Date;
        }
        if is_slash_date(t) {
            return Scalar::DateSlash;
        }
    }
    if cfg.numeric_detection && !t.is_empty() {
        if t.parse::<i64>().is_ok() {
            return Scalar::Integer;
        }
        if let Ok(f) = t.parse::<f64>() {
            if f.is_finite() {
                return Scalar::Float;
            }
        }
    }
    if cfg.detect_ip && (is_ipv4(t) || is_ipv6(t)) {
        return Scalar::Ip;
    }
    Scalar::Str
}

fn digits(s: &str, n: usize) -> bool {
    s.len() == n && s.bytes().all(|b| b.is_ascii_digit())
}

/// `strict_date_optional_time`-shaped: `YYYY-MM-DD` optionally followed by
/// `THH:MM(:SS(.fff))` and an optional `Z`/`±HH:MM` offset.
fn is_iso_date(s: &str) -> bool {
    // Guard the byte split: a non-ASCII sample must never panic here.
    if s.len() < 10 || !s.is_char_boundary(10) {
        return false;
    }
    let (date, rest) = s.split_at(10);
    let d: Vec<&str> = date.split('-').collect();
    if d.len() != 3 || !digits(d[0], 4) || !digits(d[1], 2) || !digits(d[2], 2) {
        return false;
    }
    if rest.is_empty() {
        return true;
    }
    let Some(time) = rest.strip_prefix('T').or_else(|| rest.strip_prefix(' ')) else {
        return false;
    };
    // Strip a trailing zone designator before validating the clock part.
    let time = match time.strip_suffix('Z') {
        Some(t) => t,
        None => match time.rfind(['+', '-']) {
            Some(i) if i > 0 => {
                let (t, zone) = time.split_at(i);
                let z = &zone[1..];
                let ok = digits(z, 4)
                    || (z.len() == 5 && digits(&z[..2], 2) && &z[2..3] == ":" && digits(&z[3..], 2));
                if !ok {
                    return false;
                }
                t
            }
            _ => time,
        },
    };
    let (clock, frac) = match time.split_once('.') {
        Some((c, f)) => (c, Some(f)),
        None => (time, None),
    };
    if let Some(f) = frac {
        if f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    let parts: Vec<&str> = clock.split(':').collect();
    (2..=3).contains(&parts.len()) && parts.iter().all(|p| digits(p, 2))
}

/// The other format Elasticsearch's dynamic date detection accepts by default:
/// `yyyy/MM/dd`, optionally with `HH:mm:ss` and a zone.
fn is_slash_date(s: &str) -> bool {
    let (date, rest) = match s.split_once(' ') {
        Some((d, r)) => (d, r.trim()),
        None => (s, ""),
    };
    let d: Vec<&str> = date.split('/').collect();
    if d.len() != 3 || !digits(d[0], 4) || !digits(d[1], 2) || !digits(d[2], 2) {
        return false;
    }
    if rest.is_empty() {
        return true;
    }
    // `HH:mm:ss` and/or a zone such as `Z`, `+0000`, `+00:00`.
    let (clock, zone) = match rest.split_once(' ') {
        Some((c, z)) => (c, z),
        None if rest.contains(':') && rest.len() >= 8 => (rest, ""),
        None => ("", rest),
    };
    if !clock.is_empty() {
        let parts: Vec<&str> = clock.split(':').collect();
        if parts.len() != 3 || !parts.iter().all(|p| digits(p, 2)) {
            return false;
        }
    }
    if zone.is_empty() {
        return true;
    }
    if zone == "Z" {
        return true;
    }
    let Some(rest) = zone.strip_prefix(['+', '-']) else {
        return false;
    };
    digits(rest, 4)
        || (rest.len() == 5 && digits(&rest[..2], 2) && &rest[2..3] == ":" && digits(&rest[3..], 2))
}

fn is_ipv4(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.len() <= 3
                && p.bytes().all(|b| b.is_ascii_digit())
                && p.parse::<u16>().map(|n| n <= 255).unwrap_or(false)
                && (p.len() == 1 || !p.starts_with('0'))
        })
}

fn is_ipv6(s: &str) -> bool {
    if !s.contains(':') || s.len() < 2 {
        return false;
    }
    let compressed = s.matches("::").count();
    if compressed > 1 {
        return false;
    }
    let groups: Vec<&str> = s.split(':').collect();
    if groups.len() > 8 {
        return false;
    }
    if compressed == 0 && groups.len() != 8 {
        return false;
    }
    groups.iter().all(|g| {
        g.is_empty() || (g.len() <= 4 && g.bytes().all(|b| b.is_ascii_hexdigit()))
    })
}

/// Render one inferred shape as an Elasticsearch field definition. Returns
/// `None` for a field Elasticsearch would not create at all.
fn field_json(shape: &Shape, cfg: &Config) -> Option<Value> {
    match shape {
        Shape::Unknown => None,
        Shape::GeoPoint => Some(obj([("type", Value::from("geo_point"))])),
        Shape::Object {
            properties,
            in_array,
        } => {
            let mut out = Map::new();
            let kind = if *in_array && cfg.array_objects == ArrayObjects::Nested {
                "nested"
            } else {
                "object"
            };
            out.insert("type".into(), Value::from(kind));
            let props = properties_json(properties, cfg);
            if !props.is_empty() {
                out.insert("properties".into(), Value::Object(props));
            }
            Some(Value::Object(out))
        }
        Shape::Scalar(s) => Some(scalar_json(*s, cfg)),
    }
}

fn scalar_json(s: Scalar, cfg: &Config) -> Value {
    match s {
        Scalar::Boolean => obj([("type", Value::from("boolean"))]),
        Scalar::Integer => obj([("type", Value::from(cfg.integer_type))]),
        Scalar::Float => obj([("type", Value::from(cfg.float_type))]),
        Scalar::Date => obj([("type", Value::from("date"))]),
        Scalar::DateSlash => obj([
            ("type", Value::from("date")),
            (
                "format",
                Value::from("yyyy/MM/dd HH:mm:ss Z||yyyy/MM/dd Z||yyyy/MM/dd||strict_date_optional_time"),
            ),
        ]),
        Scalar::Ip => obj([("type", Value::from("ip"))]),
        Scalar::Str => string_json(cfg),
    }
}

fn string_json(cfg: &Config) -> Value {
    let mut out = Map::new();
    if cfg.text_fields.has_text() {
        out.insert("type".into(), Value::from("text"));
        if !cfg.analyzer.is_empty() {
            out.insert("analyzer".into(), Value::from(cfg.analyzer.clone()));
        }
        if cfg.text_fields.has_keyword() {
            let mut kw = Map::new();
            kw.insert("type".into(), Value::from("keyword"));
            if cfg.ignore_above > 0 {
                kw.insert("ignore_above".into(), Value::from(cfg.ignore_above));
            }
            let mut fields = Map::new();
            fields.insert("keyword".into(), Value::Object(kw));
            out.insert("fields".into(), Value::Object(fields));
        }
    } else {
        out.insert("type".into(), Value::from("keyword"));
        if cfg.ignore_above > 0 {
            out.insert("ignore_above".into(), Value::from(cfg.ignore_above));
        }
    }
    Value::Object(out)
}

fn properties_json(properties: &BTreeMap<String, Shape>, cfg: &Config) -> Map<String, Value> {
    let mut out = Map::new();
    for (name, shape) in properties {
        if let Some(field) = field_json(shape, cfg) {
            out.insert(name.clone(), field);
        }
    }
    out
}

fn obj<const N: usize>(pairs: [(&str, Value); N]) -> Value {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert(k.to_string(), v);
    }
    Value::Object(m)
}

/// Infer an Elasticsearch mapping from `json` and render it pretty-printed.
///
/// `json` must be a JSON object (one sample document) or a non-empty array of
/// JSON objects (several samples, merged).
pub fn generate(json: &str, opts: &Options) -> Result<String, String> {
    let cfg = Config::build(opts)?;

    if json.trim().is_empty() {
        return Err("no JSON provided — paste a sample document".into());
    }
    let value: Value = serde_json::from_str(json).map_err(|e| {
        format!(
            "invalid JSON at line {} column {}: {}",
            e.line(),
            e.column(),
            e
        )
    })?;

    let docs: Vec<&Value> = match &value {
        Value::Object(_) => vec![&value],
        Value::Array(items) => {
            if items.is_empty() {
                return Err(
                    "the sample array is empty — provide at least one example document".into(),
                );
            }
            for (i, item) in items.iter().enumerate() {
                if !item.is_object() {
                    return Err(format!(
                        "sample document {} is a {}, not an object — an array sample must hold JSON objects",
                        i + 1,
                        type_name(item)
                    ));
                }
            }
            items.iter().collect()
        }
        other => {
            return Err(format!(
                "the sample must be a JSON object or an array of JSON objects, got a {}",
                type_name(other)
            ))
        }
    };

    let mut root = Shape::Unknown;
    for doc in docs {
        root = root.merge(shape_of(doc, &cfg, 0)?);
    }
    let properties = match root {
        Shape::Object { properties, .. } => properties,
        // Only reachable for `{}` / all-null samples.
        _ => BTreeMap::new(),
    };
    let props = properties_json(&properties, &cfg);

    let mut mappings = Map::new();
    mappings.insert("dynamic".into(), Value::from(cfg.dynamic));
    if !cfg.date_detection {
        mappings.insert("date_detection".into(), Value::from(false));
    }
    if cfg.numeric_detection {
        mappings.insert("numeric_detection".into(), Value::from(true));
    }
    mappings.insert("properties".into(), Value::Object(props.clone()));

    let doc = match cfg.output {
        Output::Properties => obj([("properties", Value::Object(props))]),
        Output::Mappings => obj([("mappings", Value::Object(mappings))]),
        Output::CreateIndex => {
            let index = obj([
                ("number_of_shards", Value::from(cfg.shards)),
                ("number_of_replicas", Value::from(cfg.replicas)),
            ]);
            obj([
                ("settings", obj([("index", index)])),
                ("mappings", Value::Object(mappings)),
            ])
        }
    };

    serde_json::to_string_pretty(&doc).map_err(|e| format!("could not render the mapping: {e}"))
}

/// Read a checkbox value the page sends as `"true"`/`"false"`. Empty keeps the
/// surface default; anything else is treated as off (positive-truthy).
pub fn parse_bool(raw: &str, default: bool) -> bool {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

fn parse_i64(raw: &str, name: &str, default: i64) -> Result<i64, String> {
    match raw.trim() {
        "" => Ok(default),
        s => s
            .parse::<i64>()
            .map_err(|_| format!("{name} must be a whole number, got \"{s}\"")),
    }
}

/// Build [`Options`] from the raw strings the browser page hands over — one per
/// `[[input]]` in `page/meta.toml`, in that order. Enum values pass through as
/// strings (the core validates them); empty means "use the default".
#[allow(clippy::too_many_arguments)]
pub fn options_from_strings(
    output: &str,
    text_fields: &str,
    ignore_above: &str,
    analyzer: &str,
    integer_type: &str,
    float_type: &str,
    date_detection: &str,
    numeric_detection: &str,
    detect_ip: &str,
    detect_geo_point: &str,
    array_objects: &str,
    dynamic: &str,
    shards: &str,
    replicas: &str,
) -> Result<Options, String> {
    let d = Options::default();
    let pick = |raw: &str, fallback: &str| match raw.trim() {
        "" => fallback.to_string(),
        s => s.to_string(),
    };
    Ok(Options {
        output: pick(output, &d.output),
        text_fields: pick(text_fields, &d.text_fields),
        ignore_above: parse_i64(ignore_above, "ignore_above", d.ignore_above)?,
        analyzer: analyzer.trim().to_string(),
        integer_type: pick(integer_type, &d.integer_type),
        float_type: pick(float_type, &d.float_type),
        date_detection: parse_bool(date_detection, d.date_detection),
        numeric_detection: parse_bool(numeric_detection, d.numeric_detection),
        detect_ip: parse_bool(detect_ip, d.detect_ip),
        detect_geo_point: parse_bool(detect_geo_point, d.detect_geo_point),
        array_objects: pick(array_objects, &d.array_objects),
        dynamic: pick(dynamic, &d.dynamic),
        shards: parse_i64(shards, "shards", d.shards)?,
        replicas: parse_i64(replicas, "replicas", d.replicas)?,
    })
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gen(json: &str, opts: &Options) -> Value {
        serde_json::from_str(&generate(json, opts).unwrap()).unwrap()
    }

    fn props(json: &str, opts: &Options) -> Value {
        gen(json, opts)["mappings"]["properties"].clone()
    }

    #[test]
    fn maps_the_basic_json_types_like_dynamic_mapping_does() {
        let p = props(
            r#"{"name":"Ada","age":36,"score":9.5,"active":true,"joined":"2020-01-02T03:04:05Z","missing":null}"#,
            &Options::default(),
        );
        assert_eq!(p["age"], serde_json::json!({ "type": "long" }));
        assert_eq!(p["score"], serde_json::json!({ "type": "float" }));
        assert_eq!(p["active"], serde_json::json!({ "type": "boolean" }));
        assert_eq!(p["joined"], serde_json::json!({ "type": "date" }));
        assert_eq!(
            p["name"],
            serde_json::json!({
                "type": "text",
                "fields": { "keyword": { "type": "keyword", "ignore_above": 256 } }
            })
        );
        // null adds no field, exactly like Elasticsearch.
        assert!(p.get("missing").is_none());
    }

    #[test]
    fn nested_objects_and_arrays_of_objects_recurse() {
        let opts = Options {
            array_objects: "nested".into(),
            ..Options::default()
        };
        let p = props(
            r#"{"author":{"id":1},"tags":["a","b"],"items":[{"sku":"x","qty":2}]}"#,
            &opts,
        );
        assert_eq!(p["author"]["type"], "object");
        assert_eq!(p["author"]["properties"]["id"]["type"], "long");
        // A scalar array takes its element type.
        assert_eq!(p["tags"]["type"], "text");
        assert_eq!(p["items"]["type"], "nested");
        assert_eq!(p["items"]["properties"]["qty"]["type"], "long");
    }

    #[test]
    fn several_documents_merge_and_widen_conflicts() {
        let json = r#"[{"n":1,"who":"ada"},{"n":2.5,"who":"grace","extra":true},{"who":7}]"#;
        let p = props(json, &Options::default());
        // integer + float widens to the float type
        assert_eq!(p["n"]["type"], "float");
        // string + number is incompatible → string strategy
        assert_eq!(p["who"]["type"], "text");
        // a key only one document had is still mapped
        assert_eq!(p["extra"]["type"], "boolean");
    }

    #[test]
    fn string_strategies_and_analyzer_apply() {
        let kw = props(
            r#"{"code":"AB-12"}"#,
            &Options {
                text_fields: "keyword".into(),
                ignore_above: 64,
                ..Options::default()
            },
        );
        assert_eq!(
            kw["code"],
            serde_json::json!({ "type": "keyword", "ignore_above": 64 })
        );

        let txt = props(
            r#"{"body":"hello"}"#,
            &Options {
                text_fields: "text".into(),
                analyzer: "english".into(),
                ..Options::default()
            },
        );
        assert_eq!(
            txt["body"],
            serde_json::json!({ "type": "text", "analyzer": "english" })
        );

        // ignore_above = 0 drops the keyword limit entirely
        let no_limit = props(
            r#"{"body":"hello"}"#,
            &Options {
                ignore_above: 0,
                ..Options::default()
            },
        );
        assert_eq!(no_limit["body"]["fields"]["keyword"]["ignore_above"], Value::Null);
    }

    #[test]
    fn detection_toggles_change_string_types() {
        let sample = r#"{"when":"2020-01-02","count":"42","host":"10.0.0.7"}"#;

        let d = props(sample, &Options::default());
        assert_eq!(d["when"]["type"], "date");
        assert_eq!(d["count"]["type"], "text"); // numeric_detection off by default
        assert_eq!(d["host"]["type"], "text"); // ip detection off by default

        let all = props(
            sample,
            &Options {
                numeric_detection: true,
                detect_ip: true,
                ..Options::default()
            },
        );
        assert_eq!(all["when"]["type"], "date");
        assert_eq!(all["count"]["type"], "long");
        assert_eq!(all["host"]["type"], "ip");

        let off = gen(
            sample,
            &Options {
                date_detection: false,
                ..Options::default()
            },
        );
        assert_eq!(off["mappings"]["properties"]["when"]["type"], "text");
        assert_eq!(off["mappings"]["date_detection"], false);
    }

    #[test]
    fn slash_dates_get_an_explicit_format() {
        let p = props(r#"{"d":"2020/01/02 03:04:05 +0000"}"#, &Options::default());
        assert_eq!(p["d"]["type"], "date");
        assert!(p["d"]["format"].as_str().unwrap().contains("yyyy/MM/dd"));
    }

    #[test]
    fn geo_point_detection_is_opt_in() {
        let sample = r#"{"where":{"lat":48.8,"lon":2.3}}"#;
        let off = props(sample, &Options::default());
        assert_eq!(off["where"]["type"], "object");
        assert_eq!(off["where"]["properties"]["lat"]["type"], "float");

        let on = props(
            sample,
            &Options {
                detect_geo_point: true,
                ..Options::default()
            },
        );
        assert_eq!(on["where"], serde_json::json!({ "type": "geo_point" }));
    }

    #[test]
    fn output_shapes_wrap_the_properties_differently() {
        let sample = r#"{"a":1}"#;

        let bare = gen(
            sample,
            &Options {
                output: "properties".into(),
                ..Options::default()
            },
        );
        assert!(bare.get("mappings").is_none());
        assert_eq!(bare["properties"]["a"]["type"], "long");

        let create = gen(
            sample,
            &Options {
                output: "create-index".into(),
                shards: 3,
                replicas: 2,
                dynamic: "strict".into(),
                ..Options::default()
            },
        );
        assert_eq!(create["settings"]["index"]["number_of_shards"], 3);
        assert_eq!(create["settings"]["index"]["number_of_replicas"], 2);
        assert_eq!(create["mappings"]["dynamic"], "strict");
    }

    #[test]
    fn numeric_widths_are_configurable() {
        let p = props(
            r#"{"i":1,"f":1.5}"#,
            &Options {
                integer_type: "integer".into(),
                float_type: "double".into(),
                ..Options::default()
            },
        );
        assert_eq!(p["i"]["type"], "integer");
        assert_eq!(p["f"]["type"], "double");
    }

    #[test]
    fn empty_arrays_and_all_null_fields_add_no_field() {
        let p = props(r#"{"kept":1,"none":[],"nulls":[null,null]}"#, &Options::default());
        assert_eq!(p["kept"]["type"], "long");
        assert!(p.get("none").is_none());
        assert!(p.get("nulls").is_none());
    }

    #[test]
    fn invalid_json_is_reported_with_a_position() {
        let err = generate("{bad}", &Options::default()).unwrap_err();
        assert!(err.contains("invalid JSON"), "got: {err}");
        assert!(err.contains("line 1"), "got: {err}");
    }

    #[test]
    fn non_object_samples_are_rejected() {
        let err = generate("[1,2,3]", &Options::default()).unwrap_err();
        assert!(err.contains("not an object"), "got: {err}");

        let err = generate("\"hello\"", &Options::default()).unwrap_err();
        assert!(err.contains("must be a JSON object"), "got: {err}");

        let err = generate("[]", &Options::default()).unwrap_err();
        assert!(err.contains("empty"), "got: {err}");

        let err = generate("   ", &Options::default()).unwrap_err();
        assert!(err.contains("no JSON provided"), "got: {err}");
    }

    #[test]
    fn out_of_range_and_unknown_options_are_named() {
        let err = generate(
            r#"{"a":1}"#,
            &Options {
                ignore_above: 99_999,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("ignore_above"), "got: {err}");

        let err = generate(
            r#"{"a":1}"#,
            &Options {
                shards: 0,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("shards"), "got: {err}");

        let err = generate(
            r#"{"a":1}"#,
            &Options {
                text_fields: "wat".into(),
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("text_fields"), "got: {err}");

        let err = generate(
            r#"{"a":1}"#,
            &Options {
                analyzer: "my analyzer".into(),
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("analyzer"), "got: {err}");
    }

    #[test]
    fn deep_nesting_is_capped() {
        let mut s = String::new();
        for _ in 0..(MAX_DEPTH + 5) {
            s.push_str("{\"a\":");
        }
        s.push('1');
        for _ in 0..(MAX_DEPTH + 5) {
            s.push('}');
        }
        let err = generate(&s, &Options::default()).unwrap_err();
        assert!(err.contains("deeper than"), "got: {err}");
    }

    #[test]
    fn date_and_ip_recognizers_are_strict() {
        let cfg = Config::build(&Options {
            detect_ip: true,
            ..Options::default()
        })
        .unwrap();
        assert_eq!(string_scalar("2020-01-02", &cfg), Scalar::Date);
        assert_eq!(string_scalar("2020-01-02T03:04:05.123Z", &cfg), Scalar::Date);
        assert_eq!(string_scalar("2020-01-02T03:04:05+01:00", &cfg), Scalar::Date);
        assert_eq!(string_scalar("2020-1-2", &cfg), Scalar::Str);
        assert_eq!(string_scalar("not a date", &cfg), Scalar::Str);
        assert_eq!(string_scalar("2020/01/02", &cfg), Scalar::DateSlash);

        assert_eq!(string_scalar("192.168.0.1", &cfg), Scalar::Ip);
        assert_eq!(string_scalar("::1", &cfg), Scalar::Ip);
        assert_eq!(string_scalar("2001:db8::1", &cfg), Scalar::Ip);
        assert_eq!(string_scalar("256.1.1.1", &cfg), Scalar::Str);
        assert_eq!(string_scalar("1.2.3", &cfg), Scalar::Str);
        assert_eq!(string_scalar("01.2.3.4", &cfg), Scalar::Str);
    }

    #[test]
    fn options_from_strings_matches_page_marshaling() {
        // Every field supplied, exactly as the page sends them.
        let o = options_from_strings(
            "create-index",
            "keyword",
            "128",
            " english ",
            "integer",
            "double",
            "false",
            "true",
            "true",
            "true",
            "nested",
            "strict",
            "3",
            "0",
        )
        .unwrap();
        assert_eq!(o.output, "create-index");
        assert_eq!(o.text_fields, "keyword");
        assert_eq!(o.ignore_above, 128);
        assert_eq!(o.analyzer, "english");
        assert_eq!(o.integer_type, "integer");
        assert_eq!(o.float_type, "double");
        assert!(!o.date_detection);
        assert!(o.numeric_detection && o.detect_ip && o.detect_geo_point);
        assert_eq!(o.array_objects, "nested");
        assert_eq!(o.dynamic, "strict");
        assert_eq!((o.shards, o.replicas), (3, 0));

        // Empty strings fall back to every default.
        let d = options_from_strings("", "", "", "", "", "", "", "", "", "", "", "", "", "")
            .unwrap();
        let def = Options::default();
        assert_eq!(d.output, def.output);
        assert_eq!(d.ignore_above, def.ignore_above);
        assert_eq!(d.date_detection, def.date_detection);
        assert!(!d.numeric_detection);
        assert_eq!((d.shards, d.replicas), (def.shards, def.replicas));
    }

    #[test]
    fn non_numeric_page_fields_are_reported_by_name() {
        let err = options_from_strings(
            "", "", "abc", "", "", "", "", "", "", "", "", "", "", "",
        )
        .unwrap_err();
        assert!(err.contains("ignore_above"), "got: {err}");

        let err = options_from_strings("", "", "", "", "", "", "", "", "", "", "", "", "x", "")
            .unwrap_err();
        assert!(err.contains("shards"), "got: {err}");
    }

    #[test]
    fn properties_are_sorted_for_stable_diffs() {
        let out = generate(r#"{"z":1,"a":2,"m":3}"#, &Options::default()).unwrap();
        let z = out.find("\"z\"").unwrap();
        let a = out.find("\"a\"").unwrap();
        let m = out.find("\"m\"").unwrap();
        assert!(a < m && m < z, "properties should be alphabetical");
    }
}
