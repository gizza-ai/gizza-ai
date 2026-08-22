//! bson-extended-json-converter core — convert MongoDB Extended JSON (the
//! `$oid` / `$date` / `$numberLong` wrapper dialect) to and from plain JSON.
//!
//! Pure Rust (serde_json only) → runs on every backend, including the chat
//! Service Worker. No MongoDB driver, no network: it only reshapes JSON, so the
//! result can be pasted straight into an app, a fixture file, or `mongoimport`.
//!
//! Everything funnels through one intermediate model, [`Bson`]:
//!
//! ```text
//!   Extended JSON ─┐                        ┌─→ plain JSON      (to-plain)
//!                  ├─→ parse → Bson → emit ─┤
//!   plain JSON ────┘                        └─→ Extended JSON   (to-extended)
//! ```
//!
//! Because the parser recognises canonical v2, relaxed v2 *and* the legacy v1
//! wrappers, `to-extended` doubles as a canonical↔relaxed normaliser.
//!
//! Recognised wrappers: `$oid`, `$date` (canonical `{"$numberLong":…}`,
//! relaxed ISO-8601, and legacy integer millis), `$numberInt`, `$numberLong`,
//! `$numberDouble`, `$numberDecimal`, `$binary` (v2 `{base64,subType}` and
//! legacy `{"$binary":…,"$type":…}`), `$regularExpression` and legacy
//! `{"$regex":…,"$options":…}`, `$timestamp`, `$minKey`, `$maxKey`,
//! `$undefined`, `$symbol`, `$code`, `$code`+`$scope`, `$dbPointer`.
//!
//! DBRef (`$ref`/`$id`/`$db`) is deliberately *not* a wrapper: in BSON it is an
//! ordinary subdocument, so it round-trips as one (its `$id` still transcodes).

use serde_json::{Map, Number, Value};

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Conversion direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Detect the direction from the input shape.
    Auto,
    /// Extended JSON → plain JSON (unwrap the typed wrappers).
    ToPlain,
    /// Plain JSON → Extended JSON (wrap values in typed forms).
    ToExtended,
}

impl Direction {
    /// Parse the `direction` argument (`auto` | `to-plain` | `to-extended`).
    pub fn parse(s: &str) -> Result<Direction, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Direction::Auto),
            "to-plain" | "to_plain" | "to-json" | "unwrap" => Ok(Direction::ToPlain),
            "to-extended" | "to_extended" | "to-ejson" | "wrap" => Ok(Direction::ToExtended),
            other => Err(format!(
                "invalid direction {other:?}: expected \"auto\", \"to-plain\", or \"to-extended\""
            )),
        }
    }
}

/// Which Extended JSON dialect to emit when converting *to* Extended JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Readable: plain numbers and ISO-8601 dates where the spec allows.
    Relaxed,
    /// Type-preserving: every number and date carries an explicit wrapper.
    Canonical,
}

impl Mode {
    /// Parse the `mode` argument (`relaxed` | `canonical`).
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "relaxed" => Ok(Mode::Relaxed),
            "canonical" => Ok(Mode::Canonical),
            other => Err(format!(
                "invalid mode {other:?}: expected \"relaxed\" or \"canonical\""
            )),
        }
    }
}

/// How a BSON date renders when unwrapping to plain JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateFormat {
    /// An ISO-8601 / RFC-3339 UTC string, e.g. `"2024-07-20T14:30:00Z"`.
    Iso,
    /// A raw millisecond count since the Unix epoch, e.g. `1721485800000`.
    EpochMillis,
}

impl DateFormat {
    /// Parse the `date_format` argument (`iso` | `epoch-millis`).
    pub fn parse(s: &str) -> Result<DateFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "iso" | "iso8601" => Ok(DateFormat::Iso),
            "epoch-millis" | "epoch_millis" | "millis" => Ok(DateFormat::EpochMillis),
            other => Err(format!(
                "invalid date_format {other:?}: expected \"iso\" or \"epoch-millis\""
            )),
        }
    }
}

/// Everything the two emitters need beyond the direction.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Dialect for Extended JSON output.
    pub mode: Mode,
    /// Date rendering for plain JSON output.
    pub date_format: DateFormat,
    /// Promote plain 24-hex / ISO-8601 strings to `$oid` / `$date` when wrapping.
    pub detect_types: bool,
    /// Emit 64-bit ints, Decimal128 and non-finite doubles as JSON strings when unwrapping.
    pub big_numbers_as_strings: bool,
    /// Pretty-print (indent) the output.
    pub pretty: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mode: Mode::Relaxed,
            date_format: DateFormat::Iso,
            detect_types: false,
            big_numbers_as_strings: false,
            pretty: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Intermediate model
// ---------------------------------------------------------------------------

/// A BSON value, as recovered from either JSON dialect.
///
/// Documents keep an ordered `Vec` of pairs because BSON is an ordered format
/// and re-sorting a document silently changes its meaning (index prefixes,
/// command documents, `$`-operator order).
#[derive(Debug, Clone, PartialEq)]
pub enum Bson {
    Double(f64),
    Str(String),
    Doc(Vec<(String, Bson)>),
    Arr(Vec<Bson>),
    Binary { base64: String, subtype: String },
    Undefined,
    ObjectId(String),
    Bool(bool),
    DateTime(i64),
    Null,
    Regex { pattern: String, options: String },
    DbPointer { namespace: String, id: String },
    Code(String),
    CodeWithScope { code: String, scope: Vec<(String, Bson)> },
    Symbol(String),
    Int32(i32),
    Timestamp { t: u32, i: u32 },
    Int64(i64),
    Decimal128(String),
    MinKey,
    MaxKey,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Convert `input` between MongoDB Extended JSON and plain JSON.
///
/// * `input` — the JSON document (or array, or scalar) to convert.
/// * `direction` — [`Direction::Auto`] detects, otherwise forced.
/// * `opts` — see [`Options`].
pub fn convert(input: &str, direction: Direction, opts: Options) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("input is empty: paste a JSON document to convert".to_string());
    }
    let parsed: Value =
        serde_json::from_str(trimmed).map_err(|e| format!("input is not valid JSON: {e}"))?;

    let resolved = match direction {
        Direction::Auto => {
            if has_wrapper(&parsed) {
                Direction::ToPlain
            } else {
                Direction::ToExtended
            }
        }
        other => other,
    };

    // Both directions go through the same model. `detect_types` only ever fires
    // on the wrapping path — promoting strings while unwrapping would undo the
    // very work the caller asked for.
    let detect = opts.detect_types && resolved == Direction::ToExtended;
    let doc = parse_value(&parsed, detect, &mut Vec::new())?;

    let out = match resolved {
        Direction::ToPlain => emit_plain(&doc, &opts),
        Direction::ToExtended => emit_extended(&doc, opts.mode),
        Direction::Auto => unreachable!("auto is resolved before use"),
    };

    if opts.pretty {
        serde_json::to_string_pretty(&out)
    } else {
        serde_json::to_string(&out)
    }
    .map_err(|e| format!("failed to serialize output: {e}"))
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// Every wrapper key that makes a single-key (or `$code`+`$scope`) object typed.
/// `$ref`/`$id`/`$db` are absent on purpose — DBRef is a plain subdocument.
const WRAPPER_KEYS: [&str; 15] = [
    "$oid",
    "$date",
    "$numberInt",
    "$numberLong",
    "$numberDouble",
    "$numberDecimal",
    "$binary",
    "$regularExpression",
    "$regex",
    "$timestamp",
    "$minKey",
    "$maxKey",
    "$undefined",
    "$symbol",
    "$code",
];

/// True when any recognised Extended JSON wrapper appears anywhere in `v`.
fn has_wrapper(v: &Value) -> bool {
    match v {
        Value::Object(m) => {
            if m.keys().any(|k| WRAPPER_KEYS.contains(&k.as_str())) {
                return true;
            }
            m.values().any(has_wrapper)
        }
        Value::Array(a) => a.iter().any(has_wrapper),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// JSON → Bson
// ---------------------------------------------------------------------------

/// Render the JSON-pointer-ish breadcrumb used in error messages.
fn at(path: &[String]) -> String {
    if path.is_empty() {
        "the top-level value".to_string()
    } else {
        format!("`{}`", path.join("."))
    }
}

fn parse_value(v: &Value, detect: bool, path: &mut Vec<String>) -> Result<Bson, String> {
    match v {
        Value::Null => Ok(Bson::Null),
        Value::Bool(b) => Ok(Bson::Bool(*b)),
        Value::Number(n) => Ok(number_to_bson(n)),
        Value::String(s) => Ok(parse_string(s, detect)),
        Value::Array(a) => {
            let mut out = Vec::with_capacity(a.len());
            for (i, item) in a.iter().enumerate() {
                path.push(format!("[{i}]"));
                out.push(parse_value(item, detect, path)?);
                path.pop();
            }
            Ok(Bson::Arr(out))
        }
        Value::Object(m) => parse_object(m, detect, path),
    }
}

/// Relaxed-mode numbers carry no wrapper: integers narrow to int32 when they
/// fit, otherwise int64; anything else is a double.
fn number_to_bson(n: &Number) -> Bson {
    if let Some(i) = n.as_i64() {
        return match i32::try_from(i) {
            Ok(small) => Bson::Int32(small),
            Err(_) => Bson::Int64(i),
        };
    }
    if let Some(u) = n.as_u64() {
        // Above i64::MAX there is no BSON integer type left — keep the value as
        // a double rather than silently truncating it.
        return Bson::Double(u as f64);
    }
    Bson::Double(n.as_f64().unwrap_or(f64::NAN))
}

/// Plain strings stay strings unless `detect_types` asked us to guess.
fn parse_string(s: &str, detect: bool) -> Bson {
    if detect {
        if is_object_id(s) {
            return Bson::ObjectId(s.to_ascii_lowercase());
        }
        if let Some(ms) = parse_iso8601(s) {
            return Bson::DateTime(ms);
        }
    }
    Bson::Str(s.to_string())
}

fn is_object_id(s: &str) -> bool {
    s.len() == 24 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn parse_object(m: &Map<String, Value>, detect: bool, path: &mut Vec<String>) -> Result<Bson, String> {
    if let Some(b) = parse_wrapper(m, detect, path)? {
        return Ok(b);
    }
    let mut out = Vec::with_capacity(m.len());
    for (k, v) in m {
        path.push(k.clone());
        out.push((k.clone(), parse_value(v, detect, path)?));
        path.pop();
    }
    Ok(Bson::Doc(out))
}

/// Recognise a typed wrapper object, or return `Ok(None)` for a plain document.
///
/// A wrapper is a *single*-key object (plus the `$code`+`$scope`, `$binary`+`$type`
/// and `$regex`+`$options` legacy pairs). An object that merely *contains* a
/// wrapper key alongside unrelated fields is a plain document, which is what
/// keeps `{"$ref":…,"$id":…,"$db":…}` DBRefs intact.
fn parse_wrapper(
    m: &Map<String, Value>,
    detect: bool,
    path: &mut Vec<String>,
) -> Result<Option<Bson>, String> {
    let get_str = |key: &str| -> Result<String, String> {
        match m.get(key) {
            Some(Value::String(s)) => Ok(s.clone()),
            Some(other) => Err(format!(
                "{} has {key} = {other}, but {key} must be a string",
                at(path)
            )),
            None => Err(format!("{} is missing {key}", at(path))),
        }
    };

    // --- two-key legacy / scoped forms, checked before the single-key ones ---
    if m.len() == 2 && m.contains_key("$code") && m.contains_key("$scope") {
        let code = get_str("$code")?;
        let scope = match m.get("$scope") {
            Some(Value::Object(s)) => {
                path.push("$scope".to_string());
                let mut pairs = Vec::with_capacity(s.len());
                for (k, v) in s {
                    path.push(k.clone());
                    pairs.push((k.clone(), parse_value(v, detect, path)?));
                    path.pop();
                }
                path.pop();
                pairs
            }
            _ => return Err(format!("{} has $scope, which must be an object", at(path))),
        };
        return Ok(Some(Bson::CodeWithScope { code, scope }));
    }
    if m.len() == 2 && m.contains_key("$binary") && m.contains_key("$type") {
        // Legacy v1 binary: {"$binary": "<base64>", "$type": "00"}.
        let base64 = get_str("$binary")?;
        let subtype = match m.get("$type") {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) => format!("{:02x}", n.as_u64().unwrap_or(0)),
            _ => return Err(format!("{} has an unreadable $type", at(path))),
        };
        return Ok(Some(Bson::Binary {
            base64,
            subtype: normalize_subtype(&subtype),
        }));
    }
    if m.contains_key("$regex") && m.len() <= 2 && m.keys().all(|k| k == "$regex" || k == "$options")
    {
        // Legacy v1 regex: {"$regex": "…", "$options": "…"}. A $regex whose value
        // is an *object* is a query operator, not a wrapper — leave it alone.
        if let Some(Value::String(pattern)) = m.get("$regex") {
            let options = match m.get("$options") {
                Some(Value::String(o)) => o.clone(),
                None => String::new(),
                Some(other) => {
                    return Err(format!(
                        "{} has $options = {other}, but $options must be a string",
                        at(path)
                    ))
                }
            };
            return Ok(Some(Bson::Regex {
                pattern: pattern.clone(),
                options,
            }));
        }
        return Ok(None);
    }

    if m.len() != 1 {
        return Ok(None);
    }
    let (key, val) = m.iter().next().expect("len == 1");

    let b = match key.as_str() {
        "$oid" => {
            let hex = get_str("$oid")?;
            if !is_object_id(&hex) {
                return Err(format!(
                    "{} has $oid = {hex:?}, but an ObjectId must be 24 hex characters",
                    at(path)
                ));
            }
            Bson::ObjectId(hex.to_ascii_lowercase())
        }
        "$date" => Bson::DateTime(parse_date(val, path)?),
        "$numberInt" => {
            let s = get_str("$numberInt")?;
            Bson::Int32(s.trim().parse::<i32>().map_err(|_| {
                format!("{} has $numberInt = {s:?}, which is not a 32-bit integer", at(path))
            })?)
        }
        "$numberLong" => {
            let s = get_str("$numberLong")?;
            Bson::Int64(s.trim().parse::<i64>().map_err(|_| {
                format!("{} has $numberLong = {s:?}, which is not a 64-bit integer", at(path))
            })?)
        }
        "$numberDouble" => {
            let s = get_str("$numberDouble")?;
            Bson::Double(parse_double(&s).ok_or_else(|| {
                format!(
                    "{} has $numberDouble = {s:?}, which is not a number \
                     (or \"Infinity\", \"-Infinity\", \"NaN\")",
                    at(path)
                )
            })?)
        }
        "$numberDecimal" => Bson::Decimal128(get_str("$numberDecimal")?),
        "$binary" => match val {
            // v2: {"$binary": {"base64": "…", "subType": "00"}}
            Value::Object(inner) => {
                let base64 = match inner.get("base64") {
                    Some(Value::String(s)) => s.clone(),
                    _ => return Err(format!("{} has $binary without a base64 string", at(path))),
                };
                let subtype = match inner.get("subType") {
                    Some(Value::String(s)) => normalize_subtype(s),
                    None => "00".to_string(),
                    _ => return Err(format!("{} has $binary with a non-string subType", at(path))),
                };
                Bson::Binary { base64, subtype }
            }
            _ => {
                return Err(format!(
                    "{} has $binary that is neither {{base64, subType}} nor a legacy \
                     $binary/$type pair",
                    at(path)
                ))
            }
        },
        "$regularExpression" => match val {
            Value::Object(inner) => {
                let pattern = match inner.get("pattern") {
                    Some(Value::String(s)) => s.clone(),
                    _ => {
                        return Err(format!(
                            "{} has $regularExpression without a pattern string",
                            at(path)
                        ))
                    }
                };
                let options = match inner.get("options") {
                    Some(Value::String(s)) => s.clone(),
                    None => String::new(),
                    _ => {
                        return Err(format!(
                            "{} has $regularExpression with non-string options",
                            at(path)
                        ))
                    }
                };
                Bson::Regex { pattern, options }
            }
            _ => {
                return Err(format!(
                    "{} has $regularExpression, which must be {{pattern, options}}",
                    at(path)
                ))
            }
        },
        "$timestamp" => match val {
            Value::Object(inner) => {
                let t = u32_field(inner, "t", path)?;
                let i = u32_field(inner, "i", path)?;
                Bson::Timestamp { t, i }
            }
            _ => {
                return Err(format!(
                    "{} has $timestamp, which must be {{t, i}}",
                    at(path)
                ))
            }
        },
        "$minKey" => Bson::MinKey,
        "$maxKey" => Bson::MaxKey,
        "$undefined" => Bson::Undefined,
        "$symbol" => Bson::Symbol(get_str("$symbol")?),
        "$code" => Bson::Code(get_str("$code")?),
        "$dbPointer" => match val {
            Value::Object(inner) => {
                let namespace = match inner.get("$ref") {
                    Some(Value::String(s)) => s.clone(),
                    _ => return Err(format!("{} has $dbPointer without a $ref string", at(path))),
                };
                let id = match inner.get("$id") {
                    Some(Value::Object(o)) => match o.get("$oid") {
                        Some(Value::String(s)) => s.to_ascii_lowercase(),
                        _ => return Err(format!("{} has $dbPointer without an $id $oid", at(path))),
                    },
                    _ => return Err(format!("{} has $dbPointer without an $id $oid", at(path))),
                };
                Bson::DbPointer { namespace, id }
            }
            _ => {
                return Err(format!(
                    "{} has $dbPointer, which must be {{$ref, $id}}",
                    at(path)
                ))
            }
        },
        _ => return Ok(None),
    };
    Ok(Some(b))
}

fn u32_field(m: &Map<String, Value>, key: &str, path: &[String]) -> Result<u32, String> {
    match m.get(key) {
        Some(Value::Number(n)) => n
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .ok_or_else(|| format!("{} has $timestamp {key}, which must fit in 32 bits", at(path))),
        _ => Err(format!(
            "{} has $timestamp without a numeric {key}",
            at(path)
        )),
    }
}

/// `$date` accepts all three spellings: canonical `{"$numberLong": "…"}`,
/// relaxed ISO-8601, and legacy v1 integer millis.
fn parse_date(val: &Value, path: &[String]) -> Result<i64, String> {
    match val {
        Value::Object(inner) => match inner.get("$numberLong") {
            Some(Value::String(s)) => s.trim().parse::<i64>().map_err(|_| {
                format!(
                    "{} has $date $numberLong = {s:?}, which is not a 64-bit integer",
                    at(path)
                )
            }),
            _ => Err(format!(
                "{} has $date as an object, which must be {{\"$numberLong\": \"<millis>\"}}",
                at(path)
            )),
        },
        Value::String(s) => parse_iso8601(s).ok_or_else(|| {
            format!(
                "{} has $date = {s:?}, which is not an ISO-8601 date-time",
                at(path)
            )
        }),
        Value::Number(n) => n.as_i64().ok_or_else(|| {
            format!(
                "{} has a legacy $date that is not an integer millisecond count",
                at(path)
            )
        }),
        _ => Err(format!("{} has an unreadable $date", at(path))),
    }
}

/// Binary subtypes are two lowercase hex digits in v2 output.
fn normalize_subtype(s: &str) -> String {
    let t = s.trim().trim_start_matches("0x");
    match u8::from_str_radix(t, 16) {
        Ok(v) => format!("{v:02x}"),
        Err(_) => s.to_string(),
    }
}

fn parse_double(s: &str) -> Option<f64> {
    match s.trim() {
        "Infinity" => Some(f64::INFINITY),
        "-Infinity" => Some(f64::NEG_INFINITY),
        "NaN" => Some(f64::NAN),
        other => other.parse::<f64>().ok(),
    }
}

// ---------------------------------------------------------------------------
// Bson → plain JSON
// ---------------------------------------------------------------------------

fn emit_plain(b: &Bson, opts: &Options) -> Value {
    match b {
        Bson::Null => Value::Null,
        Bson::Undefined => Value::Null,
        Bson::Bool(v) => Value::Bool(*v),
        Bson::Str(s) => Value::String(s.clone()),
        Bson::Symbol(s) => Value::String(s.clone()),
        Bson::ObjectId(hex) => Value::String(hex.clone()),
        Bson::Code(c) => Value::String(c.clone()),
        Bson::MinKey => Value::String("MinKey".to_string()),
        Bson::MaxKey => Value::String("MaxKey".to_string()),
        Bson::Binary { base64, .. } => Value::String(base64.clone()),
        Bson::Regex { pattern, options } => Value::String(format!("/{pattern}/{options}")),
        Bson::Int32(i) => Value::Number((*i).into()),
        Bson::Int64(i) => {
            if opts.big_numbers_as_strings {
                Value::String(i.to_string())
            } else {
                Value::Number((*i).into())
            }
        }
        Bson::Double(d) => {
            // JSON has no Infinity/NaN literal, so those are always strings —
            // there is no lossless numeric choice to offer.
            match Number::from_f64(*d) {
                Some(n) if !opts.big_numbers_as_strings => Value::Number(n),
                _ => Value::String(format_double(*d)),
            }
        }
        Bson::Decimal128(s) => {
            if opts.big_numbers_as_strings {
                return Value::String(s.clone());
            }
            match s.parse::<f64>().ok().and_then(Number::from_f64) {
                Some(n) => Value::Number(n),
                None => Value::String(s.clone()),
            }
        }
        Bson::DateTime(ms) => match opts.date_format {
            DateFormat::Iso => match format_iso8601(*ms) {
                Some(s) => Value::String(s),
                // Outside year 0000-9999 there is no representable ISO string.
                None => Value::Number((*ms).into()),
            },
            DateFormat::EpochMillis => Value::Number((*ms).into()),
        },
        Bson::Timestamp { t, i } => {
            let mut m = Map::new();
            m.insert("t".to_string(), Value::Number((*t).into()));
            m.insert("i".to_string(), Value::Number((*i).into()));
            Value::Object(m)
        }
        Bson::DbPointer { namespace, id } => {
            let mut m = Map::new();
            m.insert("$ref".to_string(), Value::String(namespace.clone()));
            m.insert("$id".to_string(), Value::String(id.clone()));
            Value::Object(m)
        }
        Bson::CodeWithScope { code, scope } => {
            let mut m = Map::new();
            m.insert("code".to_string(), Value::String(code.clone()));
            m.insert("scope".to_string(), plain_doc(scope, opts));
            Value::Object(m)
        }
        Bson::Arr(a) => Value::Array(a.iter().map(|v| emit_plain(v, opts)).collect()),
        Bson::Doc(d) => plain_doc(d, opts),
    }
}

fn plain_doc(pairs: &[(String, Bson)], opts: &Options) -> Value {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert(k.clone(), emit_plain(v, opts));
    }
    Value::Object(m)
}

// ---------------------------------------------------------------------------
// Bson → Extended JSON
// ---------------------------------------------------------------------------

fn wrap1(key: &str, v: Value) -> Value {
    let mut m = Map::new();
    m.insert(key.to_string(), v);
    Value::Object(m)
}

fn emit_extended(b: &Bson, mode: Mode) -> Value {
    match b {
        Bson::Null => Value::Null,
        Bson::Bool(v) => Value::Bool(*v),
        Bson::Str(s) => Value::String(s.clone()),
        Bson::Undefined => wrap1("$undefined", Value::Bool(true)),
        Bson::MinKey => wrap1("$minKey", Value::Number(1.into())),
        Bson::MaxKey => wrap1("$maxKey", Value::Number(1.into())),
        Bson::ObjectId(hex) => wrap1("$oid", Value::String(hex.clone())),
        Bson::Symbol(s) => wrap1("$symbol", Value::String(s.clone())),
        Bson::Code(c) => wrap1("$code", Value::String(c.clone())),
        Bson::Decimal128(s) => wrap1("$numberDecimal", Value::String(s.clone())),
        Bson::Int32(i) => match mode {
            Mode::Relaxed => Value::Number((*i).into()),
            Mode::Canonical => wrap1("$numberInt", Value::String(i.to_string())),
        },
        Bson::Int64(i) => match mode {
            Mode::Relaxed => Value::Number((*i).into()),
            Mode::Canonical => wrap1("$numberLong", Value::String(i.to_string())),
        },
        Bson::Double(d) => match mode {
            // A non-finite double has no relaxed spelling either — the spec
            // keeps the canonical wrapper for it in both modes.
            Mode::Relaxed => match Number::from_f64(*d) {
                Some(n) => Value::Number(n),
                None => wrap1("$numberDouble", Value::String(format_double(*d))),
            },
            Mode::Canonical => wrap1("$numberDouble", Value::String(format_double(*d))),
        },
        Bson::DateTime(ms) => {
            let canonical = wrap1(
                "$date",
                wrap1("$numberLong", Value::String(ms.to_string())),
            );
            match mode {
                Mode::Canonical => canonical,
                // Relaxed only uses the ISO spelling for years 1970-9999.
                Mode::Relaxed => match format_iso8601(*ms) {
                    Some(s) if *ms >= 0 => wrap1("$date", Value::String(s)),
                    _ => canonical,
                },
            }
        }
        Bson::Binary { base64, subtype } => {
            let mut inner = Map::new();
            inner.insert("base64".to_string(), Value::String(base64.clone()));
            inner.insert("subType".to_string(), Value::String(subtype.clone()));
            wrap1("$binary", Value::Object(inner))
        }
        Bson::Regex { pattern, options } => {
            let mut inner = Map::new();
            inner.insert("pattern".to_string(), Value::String(pattern.clone()));
            inner.insert("options".to_string(), Value::String(options.clone()));
            wrap1("$regularExpression", Value::Object(inner))
        }
        Bson::Timestamp { t, i } => {
            let mut inner = Map::new();
            inner.insert("t".to_string(), Value::Number((*t).into()));
            inner.insert("i".to_string(), Value::Number((*i).into()));
            wrap1("$timestamp", Value::Object(inner))
        }
        Bson::DbPointer { namespace, id } => {
            let mut inner = Map::new();
            inner.insert("$ref".to_string(), Value::String(namespace.clone()));
            inner.insert("$id".to_string(), wrap1("$oid", Value::String(id.clone())));
            wrap1("$dbPointer", Value::Object(inner))
        }
        Bson::CodeWithScope { code, scope } => {
            let mut m = Map::new();
            m.insert("$code".to_string(), Value::String(code.clone()));
            m.insert("$scope".to_string(), extended_doc(scope, mode));
            Value::Object(m)
        }
        Bson::Arr(a) => Value::Array(a.iter().map(|v| emit_extended(v, mode)).collect()),
        Bson::Doc(d) => extended_doc(d, mode),
    }
}

fn extended_doc(pairs: &[(String, Bson)], mode: Mode) -> Value {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert(k.clone(), emit_extended(v, mode));
    }
    Value::Object(m)
}

/// Canonical `$numberDouble` text: shortest round-tripping form, but always
/// recognisable as a double (`1` would read back as an int32, so emit `1.0`).
fn format_double(d: f64) -> String {
    if d.is_nan() {
        return "NaN".to_string();
    }
    if d.is_infinite() {
        return if d > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    let s = format!("{d}");
    if s.contains(['.', 'e', 'E']) {
        s
    } else {
        format!("{s}.0")
    }
}

// ---------------------------------------------------------------------------
// Dates — hand-rolled so the core stays dependency-free and clock-free
// ---------------------------------------------------------------------------

/// Days since 1970-01-01 → (year, month, day). Hinnant's civil_from_days.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// (year, month, day) → days since 1970-01-01. Hinnant's days_from_civil.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64; // [0, 399]
    let mp = if m > 2 { m - 3 } else { m + 9 } as u64; // [0, 11]
    let doy = (153 * mp + 2) / 5 + d as u64 - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe as i64 - 719_468
}

/// Epoch millis → `YYYY-MM-DDTHH:MM:SS[.sss]Z`, or `None` outside years 0-9999.
fn format_iso8601(ms: i64) -> Option<String> {
    let days = ms.div_euclid(86_400_000);
    let rem = ms.rem_euclid(86_400_000);
    let (y, mo, d) = civil_from_days(days);
    if !(0..=9999).contains(&y) {
        return None;
    }
    let (h, mi, s, milli) = (
        rem / 3_600_000,
        (rem / 60_000) % 60,
        (rem / 1000) % 60,
        rem % 1000,
    );
    Some(if milli == 0 {
        format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
    } else {
        format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{milli:03}Z")
    })
}

/// Parse `YYYY-MM-DDTHH:MM:SS[.fff…][Z|±HH:MM|±HHMM]` into epoch millis.
///
/// Deliberately strict: a bare `2024-07-20` or a loose `20/07/2024` returns
/// `None` so `detect_types` never promotes an ambiguous string to a date.
fn parse_iso8601(s: &str) -> Option<i64> {
    let s = s.trim();
    let b = s.as_bytes();
    if b.len() < 19 || !(b[10] == b'T' || b[10] == b't' || b[10] == b' ') {
        return None;
    }
    let num = |from: usize, to: usize| -> Option<i64> { s.get(from..to)?.parse::<i64>().ok() };
    if b[4] != b'-' || b[7] != b'-' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, mi, sec) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || sec > 60 {
        return None;
    }

    let mut rest = &s[19..];
    let mut millis = 0i64;
    if let Some(frac) = rest.strip_prefix('.') {
        let digits: String = frac.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            return None;
        }
        // Truncate to millisecond precision (BSON dates are millis).
        let mut ms_digits = digits.clone();
        ms_digits.truncate(3);
        while ms_digits.len() < 3 {
            ms_digits.push('0');
        }
        millis = ms_digits.parse::<i64>().ok()?;
        rest = &rest[1 + digits.len()..];
    }

    // Offset: Z, ±HH:MM or ±HHMM. Anything else is not a date-time we trust.
    let offset_minutes = match rest {
        "" | "Z" | "z" => 0,
        other => {
            let (sign, tail) = match other.as_bytes()[0] {
                b'+' => (1i64, &other[1..]),
                b'-' => (-1i64, &other[1..]),
                _ => return None,
            };
            let digits: String = tail.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() != 4 || tail.len() > 5 {
                return None;
            }
            let oh = digits[0..2].parse::<i64>().ok()?;
            let om = digits[2..4].parse::<i64>().ok()?;
            if oh > 23 || om > 59 {
                return None;
            }
            sign * (oh * 60 + om)
        }
    };

    // Reject impossible civil dates (2024-02-31) by round-tripping the day.
    let days = days_from_civil(y, mo as u32, d as u32);
    if civil_from_days(days) != (y, mo as u32, d as u32) {
        return None;
    }
    // A leap second (:60) clamps to :59.999 rather than rolling the minute over.
    let sec = sec.min(59);
    Some((days * 86_400 + h * 3_600 + mi * 60 + sec - offset_minutes * 60) * 1000 + millis)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options {
            pretty: false,
            ..Options::default()
        }
    }

    #[test]
    fn unwraps_the_headline_types_to_plain_json() {
        let input = r#"{
            "_id": {"$oid": "507f1f77bcf86cd799439011"},
            "created": {"$date": {"$numberLong": "1721485800000"}},
            "views": {"$numberLong": "9007199254740993"},
            "name": "Ada"
        }"#;
        let out = convert(input, Direction::Auto, opts()).unwrap();
        assert_eq!(
            out,
            r#"{"_id":"507f1f77bcf86cd799439011","created":"2024-07-20T14:30:00Z","views":9007199254740993,"name":"Ada"}"#
        );
    }

    #[test]
    fn auto_detects_the_wrapping_direction_for_plain_input() {
        let out = convert(r#"{"age":30,"score":1.5}"#, Direction::Auto, opts()).unwrap();
        // Plain input → wrap; relaxed mode leaves the numbers bare.
        assert_eq!(out, r#"{"age":30,"score":1.5}"#);

        let canonical = Options {
            mode: Mode::Canonical,
            ..opts()
        };
        let out = convert(r#"{"age":30,"score":1.5}"#, Direction::Auto, canonical).unwrap();
        assert_eq!(
            out,
            r#"{"age":{"$numberInt":"30"},"score":{"$numberDouble":"1.5"}}"#
        );
    }

    #[test]
    fn to_extended_normalises_relaxed_into_canonical() {
        let input = r#"{"at":{"$date":"2024-07-20T14:30:00Z"},"n":{"$numberLong":"5"}}"#;
        let out = convert(
            input,
            Direction::ToExtended,
            Options {
                mode: Mode::Canonical,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(
            out,
            r#"{"at":{"$date":{"$numberLong":"1721485800000"}},"n":{"$numberLong":"5"}}"#
        );
    }

    #[test]
    fn accepts_legacy_v1_wrappers() {
        let input = r#"{"at":{"$date":0},"b":{"$binary":"AQIDBA==","$type":"00"},"r":{"$regex":"^a","$options":"i"}}"#;
        let out = convert(input, Direction::ToPlain, opts()).unwrap();
        assert_eq!(
            out,
            r#"{"at":"1970-01-01T00:00:00Z","b":"AQIDBA==","r":"/^a/i"}"#
        );
    }

    #[test]
    fn keeps_dbref_as_an_ordinary_subdocument() {
        let input = r#"{"owner":{"$ref":"users","$id":{"$oid":"507f1f77bcf86cd799439011"},"$db":"app"}}"#;
        let out = convert(input, Direction::ToPlain, opts()).unwrap();
        assert_eq!(
            out,
            r#"{"owner":{"$ref":"users","$id":"507f1f77bcf86cd799439011","$db":"app"}}"#
        );
    }

    #[test]
    fn preserves_document_key_order() {
        let input = r#"{"z":1,"m":2,"a":3}"#;
        let out = convert(input, Direction::ToExtended, opts()).unwrap();
        assert_eq!(out, r#"{"z":1,"m":2,"a":3}"#);
    }

    #[test]
    fn big_numbers_as_strings_keeps_precision() {
        let input = r#"{"n":{"$numberLong":"9007199254740993"},"d":{"$numberDecimal":"1.1000000000000000000001"}}"#;
        let out = convert(
            input,
            Direction::ToPlain,
            Options {
                big_numbers_as_strings: true,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(
            out,
            r#"{"n":"9007199254740993","d":"1.1000000000000000000001"}"#
        );
    }

    #[test]
    fn epoch_millis_date_format() {
        let out = convert(
            r#"{"at":{"$date":"2024-07-20T14:30:00Z"}}"#,
            Direction::ToPlain,
            Options {
                date_format: DateFormat::EpochMillis,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out, r#"{"at":1721485800000}"#);
    }

    #[test]
    fn detect_types_promotes_hex_and_iso_strings() {
        let input = r#"{"_id":"507f1f77bcf86cd799439011","at":"2024-07-20T14:30:00Z","note":"hello"}"#;
        let out = convert(
            input,
            Direction::ToExtended,
            Options {
                detect_types: true,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(
            out,
            r#"{"_id":{"$oid":"507f1f77bcf86cd799439011"},"at":{"$date":"2024-07-20T14:30:00Z"},"note":"hello"}"#
        );
        // Off by default: the same strings stay strings.
        let out = convert(input, Direction::ToExtended, opts()).unwrap();
        assert_eq!(out, input);
    }

    #[test]
    fn covers_the_long_tail_of_wrappers() {
        let input = r#"{
            "ts": {"$timestamp": {"t": 1565545664, "i": 1}},
            "min": {"$minKey": 1},
            "max": {"$maxKey": 1},
            "u": {"$undefined": true},
            "sym": {"$symbol": "s"},
            "code": {"$code": "function(){}"},
            "cws": {"$code": "f", "$scope": {"x": {"$numberInt": "1"}}},
            "dp": {"$dbPointer": {"$ref": "c", "$id": {"$oid": "507f1f77bcf86cd799439011"}}},
            "inf": {"$numberDouble": "Infinity"}
        }"#;
        let out = convert(input, Direction::ToPlain, opts()).unwrap();
        assert_eq!(
            out,
            r#"{"ts":{"t":1565545664,"i":1},"min":"MinKey","max":"MaxKey","u":null,"sym":"s","code":"function(){}","cws":{"code":"f","scope":{"x":1}},"dp":{"$ref":"c","$id":"507f1f77bcf86cd799439011"},"inf":"Infinity"}"#
        );
    }

    #[test]
    fn round_trips_a_document_through_canonical_and_back() {
        let canonical = r#"{"_id":{"$oid":"507f1f77bcf86cd799439011"},"at":{"$date":{"$numberLong":"1721485800000"}},"b":{"$binary":{"base64":"AQIDBA==","subType":"00"}},"r":{"$regularExpression":{"pattern":"^a","options":"i"}}}"#;
        let relaxed = convert(
            canonical,
            Direction::ToExtended,
            Options {
                mode: Mode::Relaxed,
                ..opts()
            },
        )
        .unwrap();
        let back = convert(
            &relaxed,
            Direction::ToExtended,
            Options {
                mode: Mode::Canonical,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(back, canonical);
    }

    #[test]
    fn top_level_arrays_convert() {
        let out = convert(
            r#"[{"$oid":"507f1f77bcf86cd799439011"},{"$numberInt":"7"}]"#,
            Direction::Auto,
            opts(),
        )
        .unwrap();
        assert_eq!(out, r#"["507f1f77bcf86cd799439011",7]"#);
    }

    #[test]
    fn pretty_is_indented() {
        let out = convert(r#"{"a":1}"#, Direction::ToExtended, Options::default()).unwrap();
        assert_eq!(out, "{\n  \"a\": 1\n}");
    }

    // --- error paths ---

    #[test]
    fn rejects_invalid_json() {
        let err = convert("{nope}", Direction::Auto, opts()).unwrap_err();
        assert!(err.starts_with("input is not valid JSON"), "got {err}");
    }

    #[test]
    fn rejects_a_malformed_object_id_with_a_path() {
        let err = convert(
            r#"{"user":{"_id":{"$oid":"abc"}}}"#,
            Direction::ToPlain,
            opts(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            "`user._id` has $oid = \"abc\", but an ObjectId must be 24 hex characters"
        );
    }

    #[test]
    fn rejects_a_non_integer_number_long() {
        let err = convert(r#"{"n":{"$numberLong":"1.5"}}"#, Direction::ToPlain, opts()).unwrap_err();
        assert_eq!(
            err,
            "`n` has $numberLong = \"1.5\", which is not a 64-bit integer"
        );
    }

    #[test]
    fn rejects_an_unparseable_date() {
        let err = convert(r#"{"at":{"$date":"yesterday"}}"#, Direction::ToPlain, opts()).unwrap_err();
        assert!(err.contains("is not an ISO-8601 date-time"), "got {err}");
    }

    #[test]
    fn rejects_empty_input() {
        let err = convert("   ", Direction::Auto, opts()).unwrap_err();
        assert!(err.starts_with("input is empty"), "got {err}");
    }

    #[test]
    fn rejects_bad_enum_values() {
        assert!(Direction::parse("sideways").is_err());
        assert!(Mode::parse("strict").is_err());
        assert!(DateFormat::parse("rfc2822").is_err());
    }

    // --- date helpers ---

    #[test]
    fn iso_parsing_handles_offsets_fractions_and_rejects_junk() {
        assert_eq!(parse_iso8601("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_iso8601("1970-01-01T00:00:00.123Z"), Some(123));
        // +02:00 is two hours ahead of UTC, so the epoch instant is earlier.
        assert_eq!(parse_iso8601("1970-01-01T02:00:00+02:00"), Some(0));
        assert_eq!(parse_iso8601("1970-01-01T02:00:00+0200"), Some(0));
        assert_eq!(parse_iso8601("2024-02-31T00:00:00Z"), None);
        assert_eq!(parse_iso8601("2024-07-20"), None);
        assert_eq!(parse_iso8601("not a date"), None);
    }

    #[test]
    fn iso_formatting_round_trips_and_bounds() {
        for ms in [0i64, 1_721_485_800_000, -1, -86_400_000, 253_402_300_799_000] {
            let s = format_iso8601(ms).expect("in range");
            assert_eq!(parse_iso8601(&s), Some(ms), "round trip {ms}");
        }
        // Year 10000+ has no ISO spelling here.
        assert_eq!(format_iso8601(253_402_300_800_000), None);
    }

    #[test]
    fn pre_epoch_dates_stay_canonical_in_relaxed_mode() {
        let out = convert(
            r#"{"at":{"$date":{"$numberLong":"-86400000"}}}"#,
            Direction::ToExtended,
            opts(),
        )
        .unwrap();
        assert_eq!(out, r#"{"at":{"$date":{"$numberLong":"-86400000"}}}"#);
    }
}
