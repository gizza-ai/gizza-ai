//! json-schema-compat-check core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Compares an OLD and a NEW JSON Schema (draft-7 style keywords) and reports whether
//! the change is safe in each direction:
//!
//! * **consumer** compatibility (a.k.a. *backward*) — does the NEW schema still accept
//!   data that was written for the OLD schema? Anything that **narrows** the accepted
//!   value set breaks it.
//! * **producer** compatibility (a.k.a. *forward*) — is data written for the NEW schema
//!   still accepted by consumers still on the OLD schema? Anything that **widens** the
//!   accepted value set breaks it.
//!
//! Narrow-vs-widen is the single axis every rule classifies against; the `direction`
//! argument then decides which findings are reported. The checker is a **keyword-level**
//! comparator, not a formal subtype prover: changes it cannot decide (regular expressions,
//! composition keywords, unresolvable `$ref`s) are reported as warnings rather than
//! silently dropped.

use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// Per-schema input cap (1 MiB). Both sides are pasted text on every surface.
pub const MAX_SCHEMA_BYTES: usize = 1_048_576;
/// Hard cap on the report length so a pathological pair can't produce endless output.
pub const MAX_FINDINGS: usize = 200;
/// Deepest subschema level compared. Also what stops a recursive `$ref` cycle.
pub const MAX_DEPTH: usize = 32;
/// Longest `$ref` → `$ref` chain followed before giving up.
const MAX_REF_HOPS: usize = 16;

/// Keywords that carry no validation meaning — never reported as a change.
const ANNOTATIONS: [&str; 12] = [
    "title",
    "description",
    "default",
    "examples",
    "$comment",
    "$id",
    "$schema",
    "$anchor",
    "readOnly",
    "writeOnly",
    "deprecated",
    "definitions",
];

/// Keywords this checker deliberately does not compare — a change to any of them is
/// reported as a warning so it is never silently ignored.
const UNCOMPARED: [&str; 16] = [
    "allOf",
    "anyOf",
    "oneOf",
    "not",
    "if",
    "then",
    "else",
    "patternProperties",
    "propertyNames",
    "dependencies",
    "dependentRequired",
    "dependentSchemas",
    "contains",
    "additionalItems",
    "unevaluatedProperties",
    "unevaluatedItems",
];

/// Severity of a single finding. Ordered so `max()` gives the overall verdict.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Severity {
    Compatible,
    Warning,
    Breaking,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Compatible => "compatible",
            Severity::Warning => "warning",
            Severity::Breaking => "breaking",
        }
    }
}

/// Which compatibility question(s) the report answers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Consumer,
    Producer,
    Both,
}

impl Direction {
    /// Accepts the tool's own vocabulary plus the registry-world synonyms
    /// (`backward`/`forward`/`full`) so a caller used to those isn't stuck.
    pub fn parse(s: &str) -> Result<Direction, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "both" | "full" => Ok(Direction::Both),
            "consumer" | "backward" => Ok(Direction::Consumer),
            "producer" | "forward" => Ok(Direction::Producer),
            other => Err(format!(
                "direction must be one of: consumer, producer, both (got {other:?})"
            )),
        }
    }
}

/// How a change moves the set of accepted values.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Effect {
    /// Fewer values accepted → old data may be rejected → breaks consumers.
    Narrows,
    /// More values accepted → new data may be rejected by old readers → breaks producers.
    Widens,
    /// Cannot be decided by keyword comparison → reported on both sides.
    Unknown,
}

/// One reported schema change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub severity: Severity,
    /// Relevant to consumer (backward) compatibility.
    pub consumer: bool,
    /// Relevant to producer (forward) compatibility.
    pub producer: bool,
    /// JSON-Pointer path into the schema document ("" = the root schema).
    pub path: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Compare two schemas and render the human-readable report.
pub fn run(
    old_schema: &str,
    new_schema: &str,
    direction: &str,
    strict_required: bool,
) -> Result<String, String> {
    let dir = Direction::parse(direction)?;
    let old_v = parse_schema(old_schema, "old_schema")?;
    let new_v = parse_schema(new_schema, "new_schema")?;

    let mut cmp = Cmp {
        old_root: &old_v,
        new_root: &new_v,
        strict_required,
        findings: Vec::new(),
        truncated: false,
    };
    cmp.compare(&old_v, &new_v, "", 0);
    Ok(render(&cmp.findings, cmp.truncated, dir, strict_required))
}

/// The findings behind [`run`], for callers that want the structured list.
pub fn analyze(
    old_schema: &str,
    new_schema: &str,
    direction: &str,
    strict_required: bool,
) -> Result<Vec<Finding>, String> {
    let dir = Direction::parse(direction)?;
    let old_v = parse_schema(old_schema, "old_schema")?;
    let new_v = parse_schema(new_schema, "new_schema")?;
    let mut cmp = Cmp {
        old_root: &old_v,
        new_root: &new_v,
        strict_required,
        findings: Vec::new(),
        truncated: false,
    };
    cmp.compare(&old_v, &new_v, "", 0);
    Ok(cmp.findings.into_iter().filter(|f| keep(f, dir)).collect())
}

fn keep(f: &Finding, dir: Direction) -> bool {
    match dir {
        Direction::Consumer => f.consumer,
        Direction::Producer => f.producer,
        Direction::Both => true,
    }
}

fn parse_schema(text: &str, name: &str) -> Result<Value, String> {
    if text.trim().is_empty() {
        return Err(format!(
            "{name} is empty — paste a JSON Schema document (for example {{\"type\":\"object\"}})."
        ));
    }
    if text.len() > MAX_SCHEMA_BYTES {
        return Err(format!(
            "{name} is {} bytes; the limit is {MAX_SCHEMA_BYTES} bytes (1 MiB).",
            text.len()
        ));
    }
    let value: Value =
        serde_json::from_str(text).map_err(|e| format!("{name} is not valid JSON: {e}"))?;
    match value {
        Value::Object(_) | Value::Bool(_) => Ok(value),
        other => Err(format!(
            "{name} must be a JSON Schema object (or the boolean schema true/false), got {}.",
            json_kind(&other)
        )),
    }
}

fn json_kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

// ---------------------------------------------------------------------------
// Comparison engine
// ---------------------------------------------------------------------------

struct Cmp<'a> {
    old_root: &'a Value,
    new_root: &'a Value,
    strict_required: bool,
    findings: Vec<Finding>,
    truncated: bool,
}

/// A subschema in comparable form. `Schema(None)` is the unconstrained schema
/// (`true`, or a missing subschema) — every accessor then reads as "keyword absent".
enum Node<'v> {
    /// `false` — accepts nothing.
    Never,
    Schema(Option<&'v Map<String, Value>>),
    Invalid(&'static str),
}

fn node(v: &Value) -> Node<'_> {
    match v {
        Value::Bool(true) => Node::Schema(None),
        Value::Bool(false) => Node::Never,
        Value::Object(m) => Node::Schema(Some(m)),
        other => Node::Invalid(json_kind(other)),
    }
}

type Sub<'a> = Option<&'a Map<String, Value>>;

fn get<'m>(m: Sub<'m>, key: &str) -> Option<&'m Value> {
    m.and_then(|m| m.get(key))
}

fn num(m: Sub<'_>, key: &str) -> Option<f64> {
    get(m, key).and_then(|v| v.as_f64())
}

fn text<'m>(m: Sub<'m>, key: &str) -> Option<&'m str> {
    get(m, key).and_then(|v| v.as_str())
}

impl<'a> Cmp<'a> {
    fn push(&mut self, effect: Effect, severity: Severity, path: &str, message: String) {
        if self.findings.len() >= MAX_FINDINGS {
            self.truncated = true;
            return;
        }
        let (consumer, producer) = match effect {
            Effect::Narrows => (true, false),
            Effect::Widens => (false, true),
            Effect::Unknown => (true, true),
        };
        self.findings.push(Finding {
            severity,
            consumer,
            producer,
            path: path.to_string(),
            message,
        });
    }

    fn compare(&mut self, old: &'a Value, new: &'a Value, path: &str, depth: usize) {
        if depth > MAX_DEPTH {
            self.push(
                Effect::Unknown,
                Severity::Warning,
                path,
                format!(
                    "comparison stopped here: the schema nests deeper than {MAX_DEPTH} levels \
                     (a recursive \"$ref\"?), so this subschema was not compared."
                ),
            );
            return;
        }

        let old = match resolve(self.old_root, old) {
            Ok(v) => v,
            Err(e) => {
                self.push(
                    Effect::Unknown,
                    Severity::Warning,
                    path,
                    format!("old schema: {e} This subschema was not compared."),
                );
                return;
            }
        };
        let new = match resolve(self.new_root, new) {
            Ok(v) => v,
            Err(e) => {
                self.push(
                    Effect::Unknown,
                    Severity::Warning,
                    path,
                    format!("new schema: {e} This subschema was not compared."),
                );
                return;
            }
        };

        let (om, nm) = match (node(old), node(new)) {
            (Node::Invalid(k), _) => {
                self.push(
                    Effect::Unknown,
                    Severity::Warning,
                    path,
                    format!("the old subschema is {k}, not an object or boolean — not compared."),
                );
                return;
            }
            (_, Node::Invalid(k)) => {
                self.push(
                    Effect::Unknown,
                    Severity::Warning,
                    path,
                    format!("the new subschema is {k}, not an object or boolean — not compared."),
                );
                return;
            }
            (Node::Never, Node::Never) => return,
            (Node::Never, Node::Schema(_)) => {
                self.push(
                    Effect::Widens,
                    Severity::Breaking,
                    path,
                    "the old schema was false (it accepted nothing) and the new schema accepts \
                     values here."
                        .into(),
                );
                return;
            }
            (Node::Schema(_), Node::Never) => {
                self.push(
                    Effect::Narrows,
                    Severity::Breaking,
                    path,
                    "the new schema is false — nothing is accepted here any more.".into(),
                );
                return;
            }
            (Node::Schema(a), Node::Schema(b)) => (a, b),
        };

        // Local keyword rules first, then recursion, so the report reads top-down.
        self.cmp_type(om, nm, path);
        self.cmp_allowed_values(om, nm, path);
        self.cmp_numbers(om, nm, path);
        self.cmp_strings(om, nm, path);
        self.cmp_arrays(om, nm, path);
        self.cmp_object(om, nm, path);
        self.cmp_uncompared(om, nm, path);
        self.recurse(om, nm, path, depth);
    }

    // --- type ------------------------------------------------------------

    fn cmp_type(&mut self, om: Sub<'a>, nm: Sub<'a>, path: &str) {
        let (o, n) = (type_set(om), type_set(nm));
        match (&o, &n) {
            (None, None) => {}
            (None, Some(n2)) => self.push(
                Effect::Narrows,
                Severity::Breaking,
                path,
                format!(
                    "\"type\" was added ({}); the old schema accepted any type here.",
                    fmt_list(n2)
                ),
            ),
            (Some(o2), None) => self.push(
                Effect::Widens,
                Severity::Breaking,
                path,
                format!(
                    "\"type\" was removed (was {}); the new schema accepts any type here.",
                    fmt_list(o2)
                ),
            ),
            (Some(o2), Some(n2)) => {
                let (oe, ne) = (expand_types(o2), expand_types(n2));
                let removed: Vec<String> = oe.difference(&ne).cloned().collect();
                let added: Vec<String> = ne.difference(&oe).cloned().collect();
                if !removed.is_empty() {
                    self.push(
                        Effect::Narrows,
                        Severity::Breaking,
                        path,
                        format!(
                            "\"type\" narrowed from {} to {}; it no longer accepts: {}.",
                            fmt_list(o2),
                            fmt_list(n2),
                            removed.join(", ")
                        ),
                    );
                }
                if !added.is_empty() {
                    self.push(
                        Effect::Widens,
                        Severity::Breaking,
                        path,
                        format!(
                            "\"type\" widened from {} to {}; it now also accepts: {}.",
                            fmt_list(o2),
                            fmt_list(n2),
                            added.join(", ")
                        ),
                    );
                }
            }
        }
    }

    // --- enum / const ----------------------------------------------------

    fn cmp_allowed_values(&mut self, om: Sub<'a>, nm: Sub<'a>, path: &str) {
        match (allowed_values(om), allowed_values(nm)) {
            (None, None) => {}
            (None, Some((kw, vs))) => self.push(
                Effect::Narrows,
                Severity::Breaking,
                path,
                format!(
                    "\"{kw}\" was added, restricting values to: {}. Values outside that list are \
                     now rejected.",
                    fmt_values(&vs)
                ),
            ),
            (Some((kw, vs)), None) => self.push(
                Effect::Widens,
                Severity::Breaking,
                path,
                format!(
                    "\"{kw}\" was removed (was: {}); any value is now accepted here.",
                    fmt_values(&vs)
                ),
            ),
            (Some((okw, ov)), Some((nkw, nv))) => {
                let removed: Vec<&Value> = ov
                    .iter()
                    .filter(|(k, _)| !nv.contains_key(*k))
                    .map(|(_, v)| *v)
                    .collect();
                let added: Vec<&Value> = nv
                    .iter()
                    .filter(|(k, _)| !ov.contains_key(*k))
                    .map(|(_, v)| *v)
                    .collect();
                let kw = if okw == nkw {
                    format!("\"{okw}\"")
                } else {
                    format!("\"{okw}\" → \"{nkw}\"")
                };
                if !removed.is_empty() {
                    self.push(
                        Effect::Narrows,
                        Severity::Breaking,
                        path,
                        format!(
                            "{kw} values were removed: {}. Data that uses them is rejected by the \
                             new schema.",
                            fmt_value_list(&removed)
                        ),
                    );
                }
                if !added.is_empty() {
                    self.push(
                        Effect::Widens,
                        Severity::Breaking,
                        path,
                        format!(
                            "{kw} values were added: {}. Consumers on the old schema reject data \
                             that uses them.",
                            fmt_value_list(&added)
                        ),
                    );
                }
            }
        }
    }

    // --- numbers ---------------------------------------------------------

    fn cmp_numbers(&mut self, om: Sub<'a>, nm: Sub<'a>, path: &str) {
        match (lower_bound(om), lower_bound(nm)) {
            (None, None) => {}
            (None, Some(b)) => self.push(
                Effect::Narrows,
                Severity::Breaking,
                path,
                format!(
                    "a lower bound was added ({}); smaller numbers are now rejected.",
                    fmt_lower(b)
                ),
            ),
            (Some(b), None) => self.push(
                Effect::Widens,
                Severity::Breaking,
                path,
                format!(
                    "the lower bound was removed (was {}); the new schema accepts smaller numbers.",
                    fmt_lower(b)
                ),
            ),
            (Some(o), Some(n)) => {
                if tighter_low(n, o) {
                    self.push(
                        Effect::Narrows,
                        Severity::Breaking,
                        path,
                        format!(
                            "the lower bound was raised from {} to {}; numbers the old schema \
                             allowed are now rejected.",
                            fmt_lower(o),
                            fmt_lower(n)
                        ),
                    );
                } else if tighter_low(o, n) {
                    self.push(
                        Effect::Widens,
                        Severity::Breaking,
                        path,
                        format!(
                            "the lower bound was lowered from {} to {}; consumers on the old \
                             schema reject the numbers this newly allows.",
                            fmt_lower(o),
                            fmt_lower(n)
                        ),
                    );
                }
            }
        }

        match (upper_bound(om), upper_bound(nm)) {
            (None, None) => {}
            (None, Some(b)) => self.push(
                Effect::Narrows,
                Severity::Breaking,
                path,
                format!(
                    "an upper bound was added ({}); larger numbers are now rejected.",
                    fmt_upper(b)
                ),
            ),
            (Some(b), None) => self.push(
                Effect::Widens,
                Severity::Breaking,
                path,
                format!(
                    "the upper bound was removed (was {}); the new schema accepts larger numbers.",
                    fmt_upper(b)
                ),
            ),
            (Some(o), Some(n)) => {
                if tighter_high(n, o) {
                    self.push(
                        Effect::Narrows,
                        Severity::Breaking,
                        path,
                        format!(
                            "the upper bound was lowered from {} to {}; numbers the old schema \
                             allowed are now rejected.",
                            fmt_upper(o),
                            fmt_upper(n)
                        ),
                    );
                } else if tighter_high(o, n) {
                    self.push(
                        Effect::Widens,
                        Severity::Breaking,
                        path,
                        format!(
                            "the upper bound was raised from {} to {}; consumers on the old schema \
                             reject the numbers this newly allows.",
                            fmt_upper(o),
                            fmt_upper(n)
                        ),
                    );
                }
            }
        }

        match (num(om, "multipleOf"), num(nm, "multipleOf")) {
            (None, None) => {}
            (None, Some(n)) => self.push(
                Effect::Narrows,
                Severity::Breaking,
                path,
                format!(
                    "\"multipleOf\" was added ({}); numbers that are not a multiple are rejected.",
                    fmt_num(n)
                ),
            ),
            (Some(o), None) => self.push(
                Effect::Widens,
                Severity::Breaking,
                path,
                format!(
                    "\"multipleOf\" was removed (was {}); any number is now accepted.",
                    fmt_num(o)
                ),
            ),
            (Some(o), Some(n)) if o != n => {
                if is_multiple(n, o) {
                    self.push(
                        Effect::Narrows,
                        Severity::Breaking,
                        path,
                        format!(
                            "\"multipleOf\" was tightened from {} to {}.",
                            fmt_num(o),
                            fmt_num(n)
                        ),
                    );
                } else if is_multiple(o, n) {
                    self.push(
                        Effect::Widens,
                        Severity::Breaking,
                        path,
                        format!(
                            "\"multipleOf\" was loosened from {} to {}.",
                            fmt_num(o),
                            fmt_num(n)
                        ),
                    );
                } else {
                    self.push(
                        Effect::Unknown,
                        Severity::Warning,
                        path,
                        format!(
                            "\"multipleOf\" changed from {} to {}; neither is a multiple of the \
                             other, so acceptance changes in both directions.",
                            fmt_num(o),
                            fmt_num(n)
                        ),
                    );
                }
            }
            _ => {}
        }
    }

    // --- strings ---------------------------------------------------------

    fn cmp_strings(&mut self, om: Sub<'a>, nm: Sub<'a>, path: &str) {
        self.cmp_count_bound(om, nm, path, "minLength", true, "strings");
        self.cmp_count_bound(om, nm, path, "maxLength", false, "strings");

        match (text(om, "pattern"), text(nm, "pattern")) {
            (None, None) => {}
            (None, Some(p)) => self.push(
                Effect::Narrows,
                Severity::Breaking,
                path,
                format!(
                    "\"pattern\" was added ({p:?}); strings that do not match are now rejected."
                ),
            ),
            (Some(p), None) => self.push(
                Effect::Widens,
                Severity::Breaking,
                path,
                format!(
                    "\"pattern\" was removed (was {p:?}); the new schema accepts strings the old \
                     one rejected."
                ),
            ),
            (Some(o), Some(n)) if o != n => self.push(
                Effect::Unknown,
                Severity::Warning,
                path,
                format!(
                    "\"pattern\" changed from {o:?} to {n:?}; this checker does not compare \
                     regular expressions, so review the change by hand."
                ),
            ),
            _ => {}
        }

        match (text(om, "format"), text(nm, "format")) {
            (None, None) => {}
            (None, Some(f)) => self.push(
                Effect::Narrows,
                Severity::Warning,
                path,
                format!(
                    "\"format\" was added ({f:?}); validators that enforce formats will reject \
                     values that do not match."
                ),
            ),
            (Some(f), None) => self.push(
                Effect::Widens,
                Severity::Warning,
                path,
                format!(
                    "\"format\" was removed (was {f:?}); consumers on the old schema may reject \
                     values that no longer follow it."
                ),
            ),
            (Some(o), Some(n)) if o != n => self.push(
                Effect::Unknown,
                Severity::Warning,
                path,
                format!("\"format\" changed from {o:?} to {n:?}; check both sides by hand."),
            ),
            _ => {}
        }
    }

    // --- arrays ----------------------------------------------------------

    fn cmp_arrays(&mut self, om: Sub<'a>, nm: Sub<'a>, path: &str) {
        self.cmp_count_bound(om, nm, path, "minItems", true, "arrays");
        self.cmp_count_bound(om, nm, path, "maxItems", false, "arrays");

        let (o, n) = (
            get(om, "uniqueItems") == Some(&Value::Bool(true)),
            get(nm, "uniqueItems") == Some(&Value::Bool(true)),
        );
        if !o && n {
            self.push(
                Effect::Narrows,
                Severity::Breaking,
                path,
                "\"uniqueItems\" was turned on; arrays containing duplicates are now rejected."
                    .into(),
            );
        } else if o && !n {
            self.push(
                Effect::Widens,
                Severity::Breaking,
                path,
                "\"uniqueItems\" was turned off; consumers on the old schema reject arrays with \
                 duplicates."
                    .into(),
            );
        }
    }

    /// `minLength`/`maxLength`/`minItems`/`maxItems`/`minProperties`/`maxProperties`:
    /// raising a lower bound narrows, raising an upper bound widens.
    fn cmp_count_bound(
        &mut self,
        om: Sub<'a>,
        nm: Sub<'a>,
        path: &str,
        key: &str,
        is_lower: bool,
        subject: &str,
    ) {
        let (o, n) = (num(om, key), num(nm, key));
        let narrow_clause = format!("{subject} that satisfied the old schema can now be rejected.");
        let widen_clause =
            format!("consumers on the old schema reject the {subject} this newly allows.");
        match (o, n) {
            (None, None) => {}
            (None, Some(v)) => {
                if is_lower && v <= 0.0 {
                    return; // a lower bound of 0 constrains nothing
                }
                self.push(
                    Effect::Narrows,
                    Severity::Breaking,
                    path,
                    format!("\"{key}\" was added ({}); {narrow_clause}", fmt_num(v)),
                );
            }
            (Some(v), None) => {
                if is_lower && v <= 0.0 {
                    return;
                }
                self.push(
                    Effect::Widens,
                    Severity::Breaking,
                    path,
                    format!("\"{key}\" was removed (was {}); {widen_clause}", fmt_num(v)),
                );
            }
            (Some(o), Some(n)) if o != n => {
                let raised = n > o;
                let effect = if raised == is_lower {
                    Effect::Narrows
                } else {
                    Effect::Widens
                };
                let clause = if effect == Effect::Narrows {
                    &narrow_clause
                } else {
                    &widen_clause
                };
                self.push(
                    effect,
                    Severity::Breaking,
                    path,
                    format!(
                        "\"{key}\" was {} from {} to {}; {clause}",
                        if raised { "raised" } else { "lowered" },
                        fmt_num(o),
                        fmt_num(n)
                    ),
                );
            }
            _ => {}
        }
    }

    // --- objects ---------------------------------------------------------

    fn cmp_object(&mut self, om: Sub<'a>, nm: Sub<'a>, path: &str) {
        self.cmp_count_bound(om, nm, path, "minProperties", true, "objects");
        self.cmp_count_bound(om, nm, path, "maxProperties", false, "objects");

        let (oreq, nreq) = (required_set(om), required_set(nm));
        let (oprops, nprops) = (props(om), props(nm));

        for key in nreq.difference(&oreq) {
            let mitigated = !self.strict_required && has_default(nprops, key);
            let mut message = format!(
                "property {key:?} was added to \"required\"; data written for the old schema may \
                 omit it, so the new schema rejects it."
            );
            if mitigated {
                message.push_str(
                    " The new schema declares a \"default\" for this property, so it is reported \
                     as a warning — turn on strict required-field checks to treat it as breaking.",
                );
            }
            let severity = if mitigated {
                Severity::Warning
            } else {
                Severity::Breaking
            };
            self.push(Effect::Narrows, severity, &prop_path(path, key), message);
        }

        for key in oreq.difference(&nreq) {
            let mitigated = !self.strict_required && has_default(oprops, key);
            let mut message = format!(
                "property {key:?} was removed from \"required\"; data written for the new schema \
                 may omit a property consumers on the old schema demand."
            );
            if mitigated {
                message.push_str(
                    " The old schema declares a \"default\" for this property, so it is reported \
                     as a warning — turn on strict required-field checks to treat it as breaking.",
                );
            }
            let severity = if mitigated {
                Severity::Warning
            } else {
                Severity::Breaking
            };
            self.push(Effect::Widens, severity, &prop_path(path, key), message);
        }

        // Added / removed property schemas. Whether that is a narrowing or a
        // widening depends on the *other* side's content model (open vs closed).
        let mut keys: BTreeSet<&str> = BTreeSet::new();
        for p in [oprops, nprops].into_iter().flatten() {
            keys.extend(p.keys().map(|k| k.as_str()));
        }
        for key in keys {
            let pp = prop_path(path, key);
            match (get(oprops, key), get(nprops, key)) {
                (Some(o), None) => {
                    if unconstrained(o) {
                        continue;
                    }
                    match ap(nm) {
                        Ap::Closed => self.push(
                            Effect::Narrows,
                            Severity::Breaking,
                            &pp,
                            format!(
                                "property {key:?} was removed and the new schema sets \
                                 \"additionalProperties\": false; data that carries {key:?} is \
                                 now rejected."
                            ),
                        ),
                        _ => self.push(
                            Effect::Widens,
                            Severity::Breaking,
                            &pp,
                            format!(
                                "property {key:?} was removed from \"properties\"; the new schema \
                                 no longer constrains it, so data can carry values consumers on \
                                 the old schema reject."
                            ),
                        ),
                    }
                }
                (None, Some(n)) => {
                    if unconstrained(n) {
                        continue;
                    }
                    match ap(om) {
                        Ap::Closed => self.push(
                            Effect::Widens,
                            Severity::Breaking,
                            &pp,
                            format!(
                                "property {key:?} was added but the old schema sets \
                                 \"additionalProperties\": false; consumers on the old schema \
                                 reject data that carries it."
                            ),
                        ),
                        _ => self.push(
                            Effect::Narrows,
                            Severity::Warning,
                            &pp,
                            format!(
                                "property {key:?} was added with constraints the old schema did \
                                 not have; existing data whose {key:?} does not satisfy them is \
                                 now rejected."
                            ),
                        ),
                    }
                }
                _ => {}
            }
        }

        match (ap(om), ap(nm)) {
            (Ap::Open, Ap::Closed) => self.push(
                Effect::Narrows,
                Severity::Breaking,
                path,
                "\"additionalProperties\" was tightened to false; data carrying any extra \
                 property is now rejected."
                    .into(),
            ),
            (Ap::Closed, Ap::Open) => self.push(
                Effect::Widens,
                Severity::Breaking,
                path,
                "\"additionalProperties\" was relaxed from false; consumers on the old schema \
                 reject data carrying the extra properties this newly allows."
                    .into(),
            ),
            (Ap::Open, Ap::Schema(_)) => self.push(
                Effect::Narrows,
                Severity::Breaking,
                path,
                "\"additionalProperties\" now constrains extra properties with a schema; they \
                 were unconstrained before."
                    .into(),
            ),
            (Ap::Schema(_), Ap::Open) => self.push(
                Effect::Widens,
                Severity::Breaking,
                path,
                "\"additionalProperties\" no longer constrains extra properties; consumers on the \
                 old schema reject values that break the old constraint."
                    .into(),
            ),
            (Ap::Closed, Ap::Schema(_)) => self.push(
                Effect::Widens,
                Severity::Breaking,
                path,
                "\"additionalProperties\" changed from false to a schema; extra properties are \
                 now allowed and consumers on the old schema reject them."
                    .into(),
            ),
            (Ap::Schema(_), Ap::Closed) => self.push(
                Effect::Narrows,
                Severity::Breaking,
                path,
                "\"additionalProperties\" changed from a schema to false; extra properties are no \
                 longer allowed at all."
                    .into(),
            ),
            _ => {}
        }
    }

    // --- keywords this checker does not model ----------------------------

    fn cmp_uncompared(&mut self, om: Sub<'a>, nm: Sub<'a>, path: &str) {
        for key in UNCOMPARED {
            let (o, n) = (get(om, key), get(nm, key));
            if o == n {
                continue;
            }
            let what = match (o.is_some(), n.is_some()) {
                (false, true) => "was added",
                (true, false) => "was removed",
                _ => "changed",
            };
            self.push(
                Effect::Unknown,
                Severity::Warning,
                path,
                format!(
                    "\"{key}\" {what}; this checker does not compare composition or conditional \
                     keywords, so review the change by hand."
                ),
            );
        }
    }

    // --- recursion -------------------------------------------------------

    fn recurse(&mut self, om: Sub<'a>, nm: Sub<'a>, path: &str, depth: usize) {
        let (oprops, nprops) = (props(om), props(nm));
        let mut keys: BTreeSet<&str> = BTreeSet::new();
        for p in [oprops, nprops].into_iter().flatten() {
            keys.extend(p.keys().map(|k| k.as_str()));
        }
        for key in keys {
            if let (Some(o), Some(n)) = (get(oprops, key), get(nprops, key)) {
                self.compare(o, n, &prop_path(path, key), depth + 1);
            }
        }

        if let (Ap::Schema(o), Ap::Schema(n)) = (ap(om), ap(nm)) {
            self.compare(o, n, &format!("{path}/additionalProperties"), depth + 1);
        }

        match (get(om, "items"), get(nm, "items")) {
            (Some(Value::Array(oa)), Some(Value::Array(na))) => {
                if oa.len() != na.len() {
                    self.push(
                        Effect::Unknown,
                        Severity::Warning,
                        &format!("{path}/items"),
                        format!(
                            "the \"items\" tuple changed length from {} to {}; positional array \
                             entries beyond the shorter tuple were not compared.",
                            oa.len(),
                            na.len()
                        ),
                    );
                }
                for (i, (o, n)) in oa.iter().zip(na.iter()).enumerate() {
                    self.compare(o, n, &format!("{path}/items/{i}"), depth + 1);
                }
            }
            (Some(Value::Array(_)), Some(_)) | (Some(_), Some(Value::Array(_))) => self.push(
                Effect::Unknown,
                Severity::Warning,
                &format!("{path}/items"),
                "\"items\" changed between the tuple form and the single-schema form; it was not \
                 compared."
                    .into(),
            ),
            (Some(o), Some(n)) => self.compare(o, n, &format!("{path}/items"), depth + 1),
            (None, Some(n)) => {
                if !unconstrained(n) {
                    self.push(
                        Effect::Narrows,
                        Severity::Breaking,
                        &format!("{path}/items"),
                        "\"items\" was added; array elements are constrained where they were not \
                         before."
                            .into(),
                    );
                }
            }
            (Some(o), None) => {
                if !unconstrained(o) {
                    self.push(
                        Effect::Widens,
                        Severity::Breaking,
                        &format!("{path}/items"),
                        "\"items\" was removed; array elements are no longer constrained, so \
                         consumers on the old schema reject what this newly allows."
                            .into(),
                    );
                }
            }
            (None, None) => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Keyword accessors
// ---------------------------------------------------------------------------

/// Follow local `$ref`s (`#`, `#/…`). Remote refs and dangling pointers are errors the
/// caller turns into a warning.
fn resolve<'v>(root: &'v Value, node: &'v Value) -> Result<&'v Value, String> {
    let mut cur = node;
    for _ in 0..MAX_REF_HOPS {
        let target = match cur
            .as_object()
            .and_then(|m| m.get("$ref"))
            .and_then(|v| v.as_str())
        {
            Some(r) => r,
            None => return Ok(cur),
        };
        if target == "#" {
            cur = root;
            continue;
        }
        let pointer = match target.strip_prefix('#') {
            Some(p) if p.starts_with('/') => p,
            _ => {
                return Err(format!(
                    "\"$ref\": {target:?} is not a local pointer — remote and external \
                     references are not fetched."
                ))
            }
        };
        cur = root.pointer(pointer).ok_or_else(|| {
            format!("\"$ref\": {target:?} does not resolve inside the pasted document.")
        })?;
    }
    Err(format!(
        "\"$ref\" chain is longer than {MAX_REF_HOPS} hops — it looks circular."
    ))
}

fn type_set(m: Sub<'_>) -> Option<BTreeSet<String>> {
    match get(m, "type")? {
        Value::String(s) => Some([s.clone()].into_iter().collect()),
        Value::Array(a) => {
            let set: BTreeSet<String> = a
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            if set.is_empty() {
                None
            } else {
                Some(set)
            }
        }
        _ => None,
    }
}

/// `integer` is a subset of `number` — compare on the expanded sets so
/// `number` → `integer` reads as a narrowing and not as a swap.
fn expand_types(set: &BTreeSet<String>) -> BTreeSet<String> {
    let mut out = set.clone();
    if out.contains("number") {
        out.insert("integer".into());
    }
    out
}

/// `const` (as a one-value list) or `enum`, keyed by canonical JSON so the diff is
/// value-based and deterministically ordered.
fn allowed_values(m: Sub<'_>) -> Option<(&'static str, BTreeMap<String, &Value>)> {
    if let Some(c) = get(m, "const") {
        return Some(("const", [(canon(c), c)].into_iter().collect()));
    }
    match get(m, "enum")? {
        Value::Array(a) => Some(("enum", a.iter().map(|v| (canon(v), v)).collect())),
        _ => None,
    }
}

fn canon(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "?".into())
}

/// Effective lower bound as (value, exclusive). Handles the draft-7 numeric
/// `exclusiveMinimum` and the draft-4 boolean form.
fn lower_bound(m: Sub<'_>) -> Option<(f64, bool)> {
    let mut bound = num(m, "minimum").map(|v| (v, false));
    match get(m, "exclusiveMinimum") {
        Some(Value::Bool(true)) => {
            if let Some((v, _)) = bound {
                bound = Some((v, true));
            }
        }
        Some(Value::Number(n)) => {
            if let Some(v) = n.as_f64() {
                let candidate = (v, true);
                bound = Some(match bound {
                    Some(b) if tighter_low(b, candidate) => b,
                    _ => candidate,
                });
            }
        }
        _ => {}
    }
    bound
}

fn upper_bound(m: Sub<'_>) -> Option<(f64, bool)> {
    let mut bound = num(m, "maximum").map(|v| (v, false));
    match get(m, "exclusiveMaximum") {
        Some(Value::Bool(true)) => {
            if let Some((v, _)) = bound {
                bound = Some((v, true));
            }
        }
        Some(Value::Number(n)) => {
            if let Some(v) = n.as_f64() {
                let candidate = (v, true);
                bound = Some(match bound {
                    Some(b) if tighter_high(b, candidate) => b,
                    _ => candidate,
                });
            }
        }
        _ => {}
    }
    bound
}

/// Is lower bound `a` stricter than lower bound `b`?
fn tighter_low(a: (f64, bool), b: (f64, bool)) -> bool {
    a.0 > b.0 || (a.0 == b.0 && a.1 && !b.1)
}

/// Is upper bound `a` stricter than upper bound `b`?
fn tighter_high(a: (f64, bool), b: (f64, bool)) -> bool {
    a.0 < b.0 || (a.0 == b.0 && a.1 && !b.1)
}

fn is_multiple(a: f64, b: f64) -> bool {
    if b == 0.0 || !a.is_finite() || !b.is_finite() {
        return false;
    }
    let q = a / b;
    (q - q.round()).abs() < 1e-9
}

fn required_set(m: Sub<'_>) -> BTreeSet<String> {
    match get(m, "required") {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        _ => BTreeSet::new(),
    }
}

fn props(m: Sub<'_>) -> Sub<'_> {
    get(m, "properties").and_then(|v| v.as_object())
}

fn has_default(props: Sub<'_>, key: &str) -> bool {
    get(props, key)
        .and_then(|v| v.as_object())
        .is_some_and(|m| m.contains_key("default"))
}

/// `additionalProperties` in comparable form.
enum Ap<'v> {
    /// `true` or absent — extra properties allowed, unconstrained.
    Open,
    /// `false` — no extra properties.
    Closed,
    Schema(&'v Value),
}

fn ap<'v>(m: Sub<'v>) -> Ap<'v> {
    match get(m, "additionalProperties") {
        None | Some(Value::Bool(true)) => Ap::Open,
        Some(Value::Bool(false)) => Ap::Closed,
        Some(v) => Ap::Schema(v),
    }
}

/// A subschema that constrains nothing: `true`, `{}`, or annotations only.
fn unconstrained(v: &Value) -> bool {
    match v {
        Value::Bool(true) => true,
        Value::Object(m) => m.keys().all(|k| ANNOTATIONS.contains(&k.as_str())),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

fn escape_pointer(seg: &str) -> String {
    seg.replace('~', "~0").replace('/', "~1")
}

fn prop_path(path: &str, key: &str) -> String {
    format!("{path}/properties/{}", escape_pointer(key))
}

fn fmt_list(set: &BTreeSet<String>) -> String {
    set.iter()
        .map(|s| format!("{s:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn fmt_values(vals: &BTreeMap<String, &Value>) -> String {
    fmt_value_list(&vals.values().copied().collect::<Vec<_>>())
}

fn fmt_value_list(vals: &[&Value]) -> String {
    const SHOWN: usize = 8;
    let mut out: Vec<String> = vals.iter().take(SHOWN).map(|v| canon(v)).collect();
    if vals.len() > SHOWN {
        out.push(format!("… and {} more", vals.len() - SHOWN));
    }
    out.join(", ")
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn fmt_lower((v, exclusive): (f64, bool)) -> String {
    format!("{} {}", if exclusive { ">" } else { ">=" }, fmt_num(v))
}

fn fmt_upper((v, exclusive): (f64, bool)) -> String {
    format!("{} {}", if exclusive { "<" } else { "<=" }, fmt_num(v))
}

fn disp_path(path: &str) -> &str {
    if path.is_empty() {
        "/ (root schema)"
    } else {
        path
    }
}

fn worst(findings: &[&Finding]) -> Severity {
    findings
        .iter()
        .map(|f| f.severity)
        .max()
        .unwrap_or(Severity::Compatible)
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

fn render(findings: &[Finding], truncated: bool, dir: Direction, strict: bool) -> String {
    let shown: Vec<&Finding> = findings.iter().filter(|f| keep(f, dir)).collect();
    let verdict = worst(&shown);
    let breaking = shown
        .iter()
        .filter(|f| f.severity == Severity::Breaking)
        .count();
    let warnings = shown
        .iter()
        .filter(|f| f.severity == Severity::Warning)
        .count();

    let mut out = String::new();
    let _ = writeln!(out, "Verdict: {}", verdict.as_str());
    let _ = writeln!(
        out,
        "Direction: {}",
        match dir {
            Direction::Consumer => "consumer (backward) compatibility only",
            Direction::Producer => "producer (forward) compatibility only",
            Direction::Both => "both (consumer + producer)",
        }
    );
    let _ = writeln!(
        out,
        "Findings: {breaking} breaking, {warnings} warning{}",
        plural(warnings)
    );

    if dir != Direction::Producer {
        section(
            &mut out,
            "Consumer compatibility — will the NEW schema accept data written for the OLD schema?",
            &shown,
            |f| f.consumer,
        );
    }
    if dir != Direction::Consumer {
        section(
            &mut out,
            "Producer compatibility — will data written for the NEW schema be accepted by \
             consumers still on the OLD schema?",
            &shown,
            |f| f.producer,
        );
    }

    let _ = write!(out, "\nNotes\n");
    let _ = writeln!(
        out,
        "  - Paths are JSON Pointers into the schema document; \"/ (root schema)\" is the \
         top level."
    );
    let _ = writeln!(
        out,
        "  - Annotation-only keywords (title, description, default, examples, $comment) are \
         ignored."
    );
    let _ = writeln!(
        out,
        "  - Strict required-field checks: {}.",
        if strict {
            "on — every change to \"required\" is breaking"
        } else {
            "off — a required-list change on a property that declares a \"default\" is a warning"
        }
    );
    let _ = writeln!(
        out,
        "  - Regular expressions, composition keywords (allOf/anyOf/oneOf/not/if-then-else) and \
         remote $refs are not compared; changes to them are reported as warnings."
    );
    if truncated {
        let _ = writeln!(
            out,
            "  - Only the first {MAX_FINDINGS} findings were collected; fix these and run the \
             comparison again."
        );
    }
    out
}

fn section(out: &mut String, heading: &str, shown: &[&Finding], pick: fn(&Finding) -> bool) {
    let picked: Vec<&&Finding> = shown.iter().filter(|f| pick(f)).collect();
    let verdict = worst(&picked.iter().map(|f| **f).collect::<Vec<_>>());
    let _ = write!(out, "\n{heading}\n");
    let _ = writeln!(out, "Verdict: {}", verdict.as_str());
    if picked.is_empty() {
        let _ = writeln!(out, "  - No findings.");
        return;
    }
    for f in picked {
        let _ = writeln!(
            out,
            "  - [{}] {} — {}",
            f.severity.as_str(),
            disp_path(&f.path),
            f.message
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn verdict(report: &str) -> String {
        report
            .lines()
            .next()
            .unwrap()
            .trim_start_matches("Verdict: ")
            .to_string()
    }

    const BASE: &str = r#"{
        "type": "object",
        "required": ["id"],
        "properties": {
            "id": { "type": "string" },
            "score": { "type": "number", "minimum": 0, "maximum": 100 },
            "status": { "type": "string", "enum": ["new", "open", "closed"] }
        }
    }"#;

    #[test]
    fn identical_schemas_are_compatible() {
        let out = run(BASE, BASE, "both", false).unwrap();
        assert_eq!(verdict(&out), "compatible");
        assert!(out.contains("Findings: 0 breaking, 0 warnings"), "{out}");
        assert_eq!(out.matches("- No findings.").count(), 2, "{out}");
    }

    #[test]
    fn annotation_only_edits_are_not_changes() {
        let new = r#"{
            "title": "Ticket",
            "description": "a ticket",
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": { "type": "string", "description": "the id" },
                "score": { "type": "number", "minimum": 0, "maximum": 100 },
                "status": { "type": "string", "enum": ["new", "open", "closed"] }
            }
        }"#;
        let out = run(BASE, new, "both", false).unwrap();
        assert_eq!(verdict(&out), "compatible", "{out}");
    }

    #[test]
    fn adding_an_optional_property_is_safe_for_consumers() {
        let new = r#"{
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": { "type": "string" },
                "score": { "type": "number", "minimum": 0, "maximum": 100 },
                "status": { "type": "string", "enum": ["new", "open", "closed"] },
                "note": { "type": "string" }
            }
        }"#;
        // The new property only constrains data the old schema left free: a warning
        // for consumers, and nothing at all for producers under an open content model.
        let consumer = run(BASE, new, "consumer", false).unwrap();
        assert_eq!(verdict(&consumer), "warning", "{consumer}");
        assert!(consumer.contains("/properties/note"), "{consumer}");
    }

    #[test]
    fn consumer_breaking_added_required_property() {
        let new = r#"{
            "type": "object",
            "required": ["id", "email"],
            "properties": {
                "id": { "type": "string" },
                "email": { "type": "string" },
                "score": { "type": "number", "minimum": 0, "maximum": 100 },
                "status": { "type": "string", "enum": ["new", "open", "closed"] }
            }
        }"#;
        let out = run(BASE, new, "consumer", false).unwrap();
        assert_eq!(verdict(&out), "breaking", "{out}");
        assert!(
            out.contains(
                "[breaking] /properties/email — property \"email\" was added to \"required\""
            ),
            "{out}"
        );
        // Producers are unaffected: the new data carries strictly more.
        let producer = run(BASE, new, "producer", false).unwrap();
        assert_eq!(verdict(&producer), "compatible", "{producer}");
    }

    #[test]
    fn added_required_property_with_a_default_is_a_warning_unless_strict() {
        let new = r#"{
            "type": "object",
            "required": ["id", "email"],
            "properties": {
                "id": { "type": "string" },
                "email": { "type": "string", "default": "" },
                "score": { "type": "number", "minimum": 0, "maximum": 100 },
                "status": { "type": "string", "enum": ["new", "open", "closed"] }
            }
        }"#;
        let lenient = run(BASE, new, "consumer", false).unwrap();
        assert_eq!(verdict(&lenient), "warning", "{lenient}");
        assert!(lenient.contains("declares a \"default\""), "{lenient}");

        let strict = run(BASE, new, "consumer", true).unwrap();
        assert_eq!(verdict(&strict), "breaking", "{strict}");
        assert!(!strict.contains("declares a \"default\""), "{strict}");
    }

    #[test]
    fn consumer_breaking_enum_narrowed() {
        let new = BASE.replace(r#"["new", "open", "closed"]"#, r#"["new", "open"]"#);
        let out = run(BASE, &new, "consumer", false).unwrap();
        assert_eq!(verdict(&out), "breaking", "{out}");
        assert!(
            out.contains("/properties/status — \"enum\" values were removed: \"closed\""),
            "{out}"
        );
        assert_eq!(
            verdict(&run(BASE, &new, "producer", false).unwrap()),
            "compatible"
        );
    }

    #[test]
    fn consumer_breaking_numeric_bound_tightened() {
        let new = BASE.replace(r#""minimum": 0"#, r#""minimum": 10"#);
        let out = run(BASE, &new, "consumer", false).unwrap();
        assert_eq!(verdict(&out), "breaking", "{out}");
        assert!(
            out.contains("/properties/score — the lower bound was raised from >= 0 to >= 10"),
            "{out}"
        );

        // Lowering the ceiling is the same kind of narrowing.
        let lower_max = BASE.replace(r#""maximum": 100"#, r#""maximum": 50"#);
        let out = run(BASE, &lower_max, "consumer", false).unwrap();
        assert!(
            out.contains("the upper bound was lowered from <= 100 to <= 50"),
            "{out}"
        );
    }

    #[test]
    fn exclusive_bound_at_the_same_value_is_a_narrowing() {
        let old = r#"{ "type": "number", "minimum": 0 }"#;
        let new = r#"{ "type": "number", "exclusiveMinimum": 0 }"#;
        let out = run(old, new, "consumer", false).unwrap();
        assert_eq!(verdict(&out), "breaking", "{out}");
        assert!(out.contains("raised from >= 0 to > 0"), "{out}");
    }

    #[test]
    fn producer_breaking_removed_required_property() {
        let new = r#"{
            "type": "object",
            "required": [],
            "properties": {
                "id": { "type": "string" },
                "score": { "type": "number", "minimum": 0, "maximum": 100 },
                "status": { "type": "string", "enum": ["new", "open", "closed"] }
            }
        }"#;
        let out = run(BASE, new, "producer", false).unwrap();
        assert_eq!(verdict(&out), "breaking", "{out}");
        assert!(
            out.contains("/properties/id — property \"id\" was removed from \"required\""),
            "{out}"
        );
        assert_eq!(
            verdict(&run(BASE, new, "consumer", false).unwrap()),
            "compatible"
        );
    }

    #[test]
    fn producer_breaking_enum_widened() {
        let new = BASE.replace(
            r#"["new", "open", "closed"]"#,
            r#"["new", "open", "closed", "archived"]"#,
        );
        let out = run(BASE, &new, "producer", false).unwrap();
        assert_eq!(verdict(&out), "breaking", "{out}");
        assert!(
            out.contains("\"enum\" values were added: \"archived\""),
            "{out}"
        );
        assert_eq!(
            verdict(&run(BASE, &new, "consumer", false).unwrap()),
            "compatible"
        );
    }

    #[test]
    fn producer_breaking_numeric_bound_widened() {
        let new = BASE.replace(r#""maximum": 100"#, r#""maximum": 1000"#);
        let out = run(BASE, &new, "producer", false).unwrap();
        assert_eq!(verdict(&out), "breaking", "{out}");
        assert!(
            out.contains("the upper bound was raised from <= 100 to <= 1000"),
            "{out}"
        );
    }

    #[test]
    fn type_narrowing_and_widening_respect_integer_subset() {
        let narrowed = run(
            r#"{ "type": "number" }"#,
            r#"{ "type": "integer" }"#,
            "both",
            false,
        )
        .unwrap();
        assert_eq!(verdict(&narrowed), "breaking", "{narrowed}");
        assert!(narrowed.contains("no longer accepts: number"), "{narrowed}");

        let widened = run(
            r#"{ "type": "integer" }"#,
            r#"{ "type": "number" }"#,
            "both",
            false,
        )
        .unwrap();
        assert!(widened.contains("now also accepts: number"), "{widened}");

        // Adding "null" to the type list widens; it never narrows.
        let nullable = run(
            r#"{ "type": "string" }"#,
            r#"{ "type": ["string", "null"] }"#,
            "consumer",
            false,
        )
        .unwrap();
        assert_eq!(verdict(&nullable), "compatible", "{nullable}");
    }

    #[test]
    fn string_length_and_pattern_rules() {
        let out = run(
            r#"{ "type": "string", "maxLength": 64 }"#,
            r#"{ "type": "string", "maxLength": 32, "minLength": 3, "pattern": "^[a-z]+$" }"#,
            "consumer",
            false,
        )
        .unwrap();
        assert_eq!(verdict(&out), "breaking", "{out}");
        assert!(
            out.contains("\"maxLength\" was lowered from 64 to 32"),
            "{out}"
        );
        assert!(out.contains("\"minLength\" was added (3)"), "{out}");
        assert!(
            out.contains("\"pattern\" was added (\"^[a-z]+$\")"),
            "{out}"
        );

        // A changed regex can't be decided — warning on both sides, never dropped.
        let changed = run(
            r#"{ "type": "string", "pattern": "^a+$" }"#,
            r#"{ "type": "string", "pattern": "^b+$" }"#,
            "both",
            false,
        )
        .unwrap();
        assert_eq!(verdict(&changed), "warning", "{changed}");
        assert_eq!(
            changed
                .matches("does not compare regular expressions")
                .count(),
            2
        );
    }

    #[test]
    fn additional_properties_tightened_and_relaxed() {
        let open = r#"{ "type": "object", "properties": { "a": { "type": "string" } } }"#;
        let closed = r#"{ "type": "object", "additionalProperties": false,
                          "properties": { "a": { "type": "string" } } }"#;

        let tightened = run(open, closed, "consumer", false).unwrap();
        assert_eq!(verdict(&tightened), "breaking", "{tightened}");
        assert!(
            tightened.contains("\"additionalProperties\" was tightened to false"),
            "{tightened}"
        );

        let relaxed = run(closed, open, "producer", false).unwrap();
        assert_eq!(verdict(&relaxed), "breaking", "{relaxed}");
        assert!(
            relaxed.contains("\"additionalProperties\" was relaxed from false"),
            "{relaxed}"
        );
    }

    #[test]
    fn removing_a_property_under_a_closed_model_breaks_consumers() {
        let old = r#"{ "type": "object", "additionalProperties": false,
                       "properties": { "a": { "type": "string" }, "b": { "type": "string" } } }"#;
        let new = r#"{ "type": "object", "additionalProperties": false,
                       "properties": { "a": { "type": "string" } } }"#;
        let out = run(old, new, "consumer", false).unwrap();
        assert_eq!(verdict(&out), "breaking", "{out}");
        assert!(
            out.contains("/properties/b — property \"b\" was removed and the new schema sets"),
            "{out}"
        );
    }

    #[test]
    fn adding_a_property_under_a_closed_model_breaks_producers() {
        let old = r#"{ "type": "object", "additionalProperties": false,
                       "properties": { "a": { "type": "string" } } }"#;
        let new = r#"{ "type": "object", "additionalProperties": false,
                       "properties": { "a": { "type": "string" }, "b": { "type": "string" } } }"#;
        let out = run(old, new, "producer", false).unwrap();
        assert_eq!(verdict(&out), "breaking", "{out}");
        assert!(out.contains("consumers on the old schema"), "{out}");
    }

    #[test]
    fn array_items_and_bounds_are_compared() {
        let old = r#"{ "type": "array", "items": { "type": "number" }, "maxItems": 10 }"#;
        let new = r#"{ "type": "array", "items": { "type": "integer" }, "maxItems": 5,
                       "uniqueItems": true }"#;
        let out = run(old, new, "consumer", false).unwrap();
        assert_eq!(verdict(&out), "breaking", "{out}");
        assert!(out.contains("/items — \"type\" narrowed"), "{out}");
        assert!(
            out.contains("\"maxItems\" was lowered from 10 to 5"),
            "{out}"
        );
        assert!(out.contains("\"uniqueItems\" was turned on"), "{out}");
    }

    #[test]
    fn local_refs_are_resolved_before_comparing() {
        let old = r##"{
            "$defs": { "id": { "type": "string" } },
            "type": "object",
            "properties": { "id": { "$ref": "#/$defs/id" } }
        }"##;
        let new = r##"{
            "$defs": { "id": { "type": "string", "minLength": 5 } },
            "type": "object",
            "properties": { "id": { "$ref": "#/$defs/id" } }
        }"##;
        let out = run(old, new, "consumer", false).unwrap();
        assert_eq!(verdict(&out), "breaking", "{out}");
        assert!(
            out.contains("/properties/id — \"minLength\" was added (5)"),
            "{out}"
        );
    }

    #[test]
    fn unresolvable_and_remote_refs_are_warnings_not_silence() {
        let old = r##"{ "type": "object", "properties": { "a": { "$ref": "#/$defs/missing" } } }"##;
        let new = r#"{ "type": "object", "properties": { "a": { "type": "string" } } }"#;
        let out = run(old, new, "both", false).unwrap();
        assert_eq!(verdict(&out), "warning", "{out}");
        assert!(
            out.contains("does not resolve inside the pasted document"),
            "{out}"
        );

        let remote = run(
            r#"{ "$ref": "https://example.com/s.json" }"#,
            r#"{ "type": "object" }"#,
            "both",
            false,
        )
        .unwrap();
        assert!(
            remote.contains("remote and external references are not fetched"),
            "{remote}"
        );
    }

    #[test]
    fn composition_keyword_changes_are_reported_as_warnings() {
        let old = r#"{ "type": "object" }"#;
        let new = r#"{ "type": "object", "oneOf": [ { "required": ["a"] } ] }"#;
        let out = run(old, new, "both", false).unwrap();
        assert_eq!(verdict(&out), "warning", "{out}");
        assert!(out.contains("\"oneOf\" was added"), "{out}");
    }

    #[test]
    fn boolean_schemas_are_handled() {
        let out = run(r#"{ "type": "string" }"#, "false", "consumer", false).unwrap();
        assert_eq!(verdict(&out), "breaking", "{out}");
        assert!(out.contains("nothing is accepted here any more"), "{out}");

        let widened = run("false", "true", "producer", false).unwrap();
        assert_eq!(verdict(&widened), "breaking", "{widened}");

        assert_eq!(
            verdict(&run("true", "true", "both", false).unwrap()),
            "compatible"
        );
    }

    #[test]
    fn recursive_refs_terminate_at_the_depth_cap() {
        let schema = r##"{
            "$defs": { "node": { "type": "object",
                                 "properties": { "child": { "$ref": "#/$defs/node" } } } },
            "$ref": "#/$defs/node"
        }"##;
        let out = run(schema, schema, "both", false).unwrap();
        assert_eq!(verdict(&out), "warning", "{out}");
        assert!(out.contains("comparison stopped here"), "{out}");
    }

    #[test]
    fn nested_paths_are_json_pointers() {
        let old = r#"{ "properties": { "a/b": { "properties": { "c": { "type": "string" } } } } }"#;
        let new =
            r#"{ "properties": { "a/b": { "properties": { "c": { "type": "integer" } } } } }"#;
        let out = run(old, new, "both", false).unwrap();
        assert!(out.contains("/properties/a~1b/properties/c"), "{out}");
    }

    #[test]
    fn direction_filters_the_report_sections() {
        let both = run(BASE, BASE, "both", false).unwrap();
        assert!(both.contains("Consumer compatibility") && both.contains("Producer compatibility"));

        let consumer = run(BASE, BASE, "consumer", false).unwrap();
        assert!(consumer.contains("Consumer compatibility"));
        assert!(!consumer.contains("Producer compatibility"), "{consumer}");

        let producer = run(BASE, BASE, "producer", false).unwrap();
        assert!(!producer.contains("Consumer compatibility"), "{producer}");
        assert!(producer.contains("Producer compatibility"));
    }

    #[test]
    fn output_is_deterministic() {
        let new = r#"{
            "type": "object",
            "required": ["id", "email"],
            "properties": {
                "zeta": { "type": "string", "minLength": 2 },
                "email": { "type": "string" },
                "alpha": { "enum": [3, 1, 2] },
                "id": { "type": "string" }
            }
        }"#;
        let first = run(BASE, new, "both", false).unwrap();
        for _ in 0..5 {
            assert_eq!(run(BASE, new, "both", false).unwrap(), first);
        }
    }

    #[test]
    fn invalid_json_is_a_clear_error() {
        let err = run("{ not json }", BASE, "both", false).unwrap_err();
        assert!(err.starts_with("old_schema is not valid JSON:"), "{err}");

        let err = run(BASE, "{\"type\": }", "both", false).unwrap_err();
        assert!(err.starts_with("new_schema is not valid JSON:"), "{err}");
    }

    #[test]
    fn empty_wrong_shape_and_oversized_inputs_are_errors() {
        assert!(run("   ", BASE, "both", false)
            .unwrap_err()
            .contains("old_schema is empty"));
        assert!(run(BASE, "", "both", false)
            .unwrap_err()
            .contains("new_schema is empty"));
        assert!(run("[1,2]", BASE, "both", false)
            .unwrap_err()
            .contains("must be a JSON Schema object"));

        let huge = format!("{{\"description\":\"{}\"}}", "x".repeat(MAX_SCHEMA_BYTES));
        assert!(run(&huge, BASE, "both", false)
            .unwrap_err()
            .contains("the limit is"));
    }

    #[test]
    fn unknown_direction_is_an_error_and_synonyms_work() {
        let err = run(BASE, BASE, "sideways", false).unwrap_err();
        assert!(
            err.contains("direction must be one of: consumer, producer, both"),
            "{err}"
        );
        assert_eq!(Direction::parse("BACKWARD").unwrap(), Direction::Consumer);
        assert_eq!(Direction::parse("forward").unwrap(), Direction::Producer);
        assert_eq!(Direction::parse("").unwrap(), Direction::Both);
    }

    #[test]
    fn analyze_returns_structured_findings() {
        let new = BASE.replace(r#"["new", "open", "closed"]"#, r#"["new", "open"]"#);
        let findings = analyze(BASE, &new, "consumer", false).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Breaking);
        assert_eq!(findings[0].path, "/properties/status");
        assert!(findings[0].consumer && !findings[0].producer);
    }

    #[test]
    fn report_is_capped_at_max_findings() {
        let mut old = String::from("{\"type\":\"object\",\"properties\":{");
        let mut new = old.clone();
        for i in 0..(MAX_FINDINGS + 20) {
            if i > 0 {
                old.push(',');
                new.push(',');
            }
            let _ = write!(old, "\"p{i}\":{{\"type\":\"string\"}}");
            let _ = write!(new, "\"p{i}\":{{\"type\":\"integer\"}}");
        }
        old.push_str("}}");
        new.push_str("}}");
        let out = run(&old, &new, "both", false).unwrap();
        assert!(
            out.contains(&format!("Only the first {MAX_FINDINGS} findings")),
            "{out}"
        );
    }
}
