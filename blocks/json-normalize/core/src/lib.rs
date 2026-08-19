//! json-normalize core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Flattens a deeply nested JSON document into normalized entity tables keyed by
//! id, driven by a small entity schema: every nested entity is lifted into
//! `entities.<type>.<id>` and each place it appeared is replaced by its id.
//! Re-running the tool on already-normalized output is a no-op, so the operation
//! is idempotent.

use serde_json::{Map, Value};

/// Largest source document accepted, in bytes.
pub const MAX_JSON_BYTES: usize = 5_000_000;
/// Largest schema accepted, in bytes.
pub const MAX_SCHEMA_BYTES: usize = 100_000;
/// Largest number of entity types accepted.
pub const MAX_ENTITY_TYPES: usize = 200;
/// Largest number of extracted entity records accepted (across all tables).
pub const MAX_ENTITIES: usize = 200_000;
/// Largest nesting depth walked before giving up.
pub const MAX_DEPTH: usize = 100;

// ---------------------------------------------------------------- schema ----

/// One relation: the entity type a field points at, and whether it is a list.
#[derive(Debug, Clone)]
struct Relation {
    /// Field path inside the parent entity, split on `.` (`meta.author`).
    path: Vec<String>,
    /// Index into `Schema::entities`.
    target: usize,
    /// True when the field holds many entities (`[users]` / `["users"]`).
    list: bool,
}

#[derive(Debug, Clone)]
struct EntityDef {
    key: String,
    /// Declared relations, in declaration order.
    relations: Vec<Relation>,
    /// Candidate id field names, first match wins.
    id_fields: Vec<String>,
}

#[derive(Debug, Clone)]
struct Schema {
    entities: Vec<EntityDef>,
}

impl Schema {
    fn index_of(&self, key: &str) -> Option<usize> {
        self.entities.iter().position(|e| e.key == key)
    }
}

/// Parse the entity schema from either the JSON object form or the shorthand
/// line form. Relation targets are resolved after every entity is known, so
/// forward references and cycles are both fine.
fn parse_schema(src: &str) -> Result<Schema, String> {
    let text = src.trim();
    if text.is_empty() {
        return Err("schema is empty: name at least the root entity, e.g. \"articles: author -> users\" or {\"articles\":{\"author\":\"users\"}}".into());
    }
    if text.len() > MAX_SCHEMA_BYTES {
        return Err(format!(
            "schema is {} bytes; the maximum is {MAX_SCHEMA_BYTES}",
            text.len()
        ));
    }
    // (entity key, [(field path, target name, is list)])
    let raw: Vec<(String, Vec<(String, String, bool)>)> = if text.starts_with('{') {
        parse_schema_json(text)?
    } else {
        parse_schema_lines(text)?
    };
    if raw.is_empty() {
        return Err("schema declares no entities".into());
    }
    if raw.len() > MAX_ENTITY_TYPES {
        return Err(format!(
            "schema declares {} entity types; the maximum is {MAX_ENTITY_TYPES}",
            raw.len()
        ));
    }
    for (i, (key, _)) in raw.iter().enumerate() {
        if raw.iter().take(i).any(|(k, _)| k == key) {
            return Err(format!("entity \"{key}\" is declared twice in the schema"));
        }
    }
    let names: Vec<&str> = raw.iter().map(|(k, _)| k.as_str()).collect();
    let mut entities = Vec::with_capacity(raw.len());
    for (key, fields) in &raw {
        let mut relations = Vec::with_capacity(fields.len());
        for (field, target, list) in fields {
            let target_idx = names.iter().position(|n| n == target).ok_or_else(|| {
                format!(
                    "field \"{field}\" of entity \"{key}\" points at unknown entity \"{target}\"; declared entities: {}",
                    names.join(", ")
                )
            })?;
            let path: Vec<String> = field
                .split('.')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if path.is_empty() {
                return Err(format!("entity \"{key}\" has a relation with an empty field name"));
            }
            relations.push(Relation {
                path,
                target: target_idx,
                list: *list,
            });
        }
        entities.push(EntityDef {
            key: key.clone(),
            relations,
            id_fields: vec!["id".to_string()],
        });
    }
    Ok(Schema { entities })
}

/// `{"articles": {"author": "users", "comments": ["comments"]}, "users": {}}`
fn parse_schema_json(text: &str) -> Result<Vec<(String, Vec<(String, String, bool)>)>, String> {
    let v: Value = serde_json::from_str(text)
        .map_err(|e| format!("schema is not valid JSON: {e}. Use {{\"articles\":{{\"author\":\"users\"}}}} or the shorthand form \"articles: author -> users\""))?;
    let obj = v
        .as_object()
        .ok_or("schema JSON must be an object of entity name -> fields")?;
    let mut out = Vec::with_capacity(obj.len());
    for (key, def) in obj {
        let key = key.trim().to_string();
        if key.is_empty() {
            return Err("schema contains an entity with an empty name".into());
        }
        let mut fields = Vec::new();
        match def {
            Value::Object(m) => {
                for (field, target) in m {
                    let (name, list) = match target {
                        Value::String(s) => (s.trim().to_string(), false),
                        Value::Array(a) if a.len() == 1 => match &a[0] {
                            Value::String(s) => (s.trim().to_string(), true),
                            _ => {
                                return Err(format!(
                                    "field \"{field}\" of entity \"{key}\" must name an entity, e.g. [\"users\"]"
                                ))
                            }
                        },
                        _ => {
                            return Err(format!(
                                "field \"{field}\" of entity \"{key}\" must be an entity name (\"users\") or a one-element array ([\"users\"])"
                            ))
                        }
                    };
                    if name.is_empty() {
                        return Err(format!(
                            "field \"{field}\" of entity \"{key}\" points at an empty entity name"
                        ));
                    }
                    fields.push((field.clone(), name, list));
                }
            }
            Value::Null => {}
            _ => {
                return Err(format!(
                    "entity \"{key}\" must map to an object of field -> entity (use {{}} for an entity with no nested entities)"
                ))
            }
        }
        out.push((key, fields));
    }
    Ok(out)
}

/// ```text
/// articles: author -> users, comments -> [comments]
/// comments: commenter -> users
/// users:
/// ```
fn parse_schema_lines(text: &str) -> Result<Vec<(String, Vec<(String, String, bool)>)>, String> {
    let mut out = Vec::new();
    for (n, raw_line) in text.lines().enumerate() {
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        let (key, rest) = line.split_once(':').ok_or_else(|| {
            format!(
                "schema line {}: expected \"entity: field -> target, ...\", got \"{line}\"",
                n + 1
            )
        })?;
        let key = key.trim().to_string();
        if key.is_empty() {
            return Err(format!("schema line {}: entity name is empty", n + 1));
        }
        let mut fields = Vec::new();
        for part in rest.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (field, target) = part.split_once("->").ok_or_else(|| {
                format!(
                    "schema line {}: expected \"field -> entity\" in \"{part}\"",
                    n + 1
                )
            })?;
            let field = field.trim().to_string();
            let target = target.trim();
            let (target, list) = match target.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
                Some(inner) => (inner.trim().to_string(), true),
                None => (target.to_string(), false),
            };
            if field.is_empty() || target.is_empty() {
                return Err(format!(
                    "schema line {}: both sides of \"->\" must be filled in (\"{part}\")",
                    n + 1
                ));
            }
            fields.push((field, target, list));
        }
        out.push((key, fields));
    }
    Ok(out)
}

fn strip_comment(line: &str) -> &str {
    let cut = line
        .find('#')
        .into_iter()
        .chain(line.find("//"))
        .min()
        .unwrap_or(line.len());
    &line[..cut]
}

/// Apply the `id_field` parameter to the parsed schema.
fn apply_id_fields(schema: &mut Schema, spec: &str) -> Result<(), String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Ok(());
    }
    if spec.starts_with('{') {
        let v: Value = serde_json::from_str(spec)
            .map_err(|e| format!("id field map is not valid JSON: {e}"))?;
        let obj = v
            .as_object()
            .ok_or("id field map must be a JSON object of entity name -> field name")?;
        let mut default: Option<Vec<String>> = None;
        for (key, val) in obj {
            let names = match val {
                Value::String(s) => split_names(s),
                Value::Array(a) => {
                    let mut v = Vec::new();
                    for item in a {
                        match item {
                            Value::String(s) => v.extend(split_names(s)),
                            _ => return Err(format!("id field map entry \"{key}\" must list field names as strings")),
                        }
                    }
                    v
                }
                _ => {
                    return Err(format!(
                        "id field map entry \"{key}\" must be a field name or a list of field names"
                    ))
                }
            };
            if names.is_empty() {
                return Err(format!("id field map entry \"{key}\" names no field"));
            }
            if key == "*" {
                default = Some(names);
                continue;
            }
            match schema.index_of(key) {
                Some(i) => schema.entities[i].id_fields = names,
                None => {
                    return Err(format!(
                        "id field map names entity \"{key}\", which is not in the schema"
                    ))
                }
            }
        }
        if let Some(names) = default {
            let listed: Vec<String> = obj.keys().cloned().collect();
            for e in schema.entities.iter_mut() {
                if !listed.iter().any(|k| k == &e.key) {
                    e.id_fields = names.clone();
                }
            }
        }
        return Ok(());
    }
    let names = split_names(spec);
    if names.is_empty() {
        return Err("id field names no field".into());
    }
    for e in schema.entities.iter_mut() {
        e.id_fields = names.clone();
    }
    Ok(())
}

fn split_names(s: &str) -> Vec<String> {
    s.split(',')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

// ----------------------------------------------------------------- enums ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MissingId {
    Error,
    Index,
    Hash,
    Keep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Conflict {
    Merge,
    Replace,
    KeepFirst,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Output {
    Normalized,
    Entities,
    Result,
    Report,
}

fn parse_enum<T: Copy>(value: &str, name: &str, table: &[(&str, T)]) -> Result<T, String> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(table[0].1);
    }
    table
        .iter()
        .find(|(k, _)| *k == v)
        .map(|(_, t)| *t)
        .ok_or_else(|| {
            format!(
                "unknown {name} \"{v}\"; choose one of: {}",
                table
                    .iter()
                    .map(|(k, _)| *k)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
}

// ------------------------------------------------------------- traversal ----

#[derive(Default, Clone)]
struct Stats {
    occurrences: usize,
    merged: usize,
    synthesized: usize,
    inline: usize,
}

struct Normalizer {
    tables: Vec<Map<String, Value>>,
    stats: Vec<Stats>,
    missing: MissingId,
    conflict: Conflict,
    total: usize,
}

impl Normalizer {
    fn new(schema: &Schema, missing: MissingId, conflict: Conflict) -> Self {
        Normalizer {
            tables: vec![Map::new(); schema.entities.len()],
            stats: vec![Stats::default(); schema.entities.len()],
            missing,
            conflict,
            total: 0,
        }
    }

    /// Normalize one value that the schema says is an entity of type `idx`.
    /// Returns the reference that replaces it (its id), or the value itself when
    /// it is already a reference / stays inline.
    fn entity(
        &mut self,
        schema: &Schema,
        idx: usize,
        value: Value,
        depth: usize,
        trail: &str,
    ) -> Result<Value, String> {
        if depth > MAX_DEPTH {
            return Err(format!(
                "document nests deeper than {MAX_DEPTH} levels at {trail}"
            ));
        }
        match value {
            // Already a bare reference (or an explicit null) — pass it through so
            // re-normalizing normalized output is a no-op.
            Value::Null | Value::String(_) | Value::Number(_) | Value::Bool(_) => Ok(value),
            Value::Array(_) => Err(format!(
                "expected a single \"{}\" object at {trail} but found an array; declare the field as a list, e.g. [{}]",
                schema.entities[idx].key, schema.entities[idx].key
            )),
            Value::Object(map) => {
                let mut obj = map;
                // Rewrite declared relations first, so nested entities are
                // extracted before this one is stored.
                for rel in &schema.entities[idx].relations {
                    let Some(slot) = path_mut(&mut obj, &rel.path) else {
                        continue;
                    };
                    let taken = slot.take();
                    let child_trail = format!("{trail}.{}", rel.path.join("."));
                    let replaced = if rel.list {
                        match taken {
                            Value::Null => Value::Null,
                            Value::Array(items) => {
                                let mut out = Vec::with_capacity(items.len());
                                for (i, item) in items.into_iter().enumerate() {
                                    out.push(self.entity(
                                        schema,
                                        rel.target,
                                        item,
                                        depth + 1,
                                        &format!("{child_trail}[{i}]"),
                                    )?);
                                }
                                Value::Array(out)
                            }
                            // Lenient: a single object where a list was declared
                            // becomes a one-element list.
                            other => Value::Array(vec![self.entity(
                                schema,
                                rel.target,
                                other,
                                depth + 1,
                                &child_trail,
                            )?]),
                        }
                    } else {
                        match taken {
                            // Lenient the other way too: many values where one
                            // was declared stay an array of references.
                            Value::Array(items) => {
                                let mut out = Vec::with_capacity(items.len());
                                for (i, item) in items.into_iter().enumerate() {
                                    out.push(self.entity(
                                        schema,
                                        rel.target,
                                        item,
                                        depth + 1,
                                        &format!("{child_trail}[{i}]"),
                                    )?);
                                }
                                Value::Array(out)
                            }
                            other => {
                                self.entity(schema, rel.target, other, depth + 1, &child_trail)?
                            }
                        }
                    };
                    // Re-borrow: the recursive calls above needed `self` mutably.
                    if let Some(slot) = path_mut(&mut obj, &rel.path) {
                        *slot = replaced;
                    }
                }

                let def = &schema.entities[idx];
                let id = def
                    .id_fields
                    .iter()
                    .find_map(|f| obj.get(f.as_str()).and_then(scalar_id));
                self.stats[idx].occurrences += 1;

                let (key, reference) = match id {
                    Some((key, reference)) => (key, reference),
                    None => match self.missing {
                        MissingId::Error => {
                            return Err(format!(
                                "\"{}\" entity at {trail} has no {} field; set an id field, or choose how to handle missing ids (index, hash, or keep)",
                                def.key,
                                def.id_fields
                                    .iter()
                                    .map(|f| format!("\"{f}\""))
                                    .collect::<Vec<_>>()
                                    .join(" / ")
                            ));
                        }
                        MissingId::Keep => {
                            self.stats[idx].inline += 1;
                            return Ok(Value::Object(obj));
                        }
                        MissingId::Index => {
                            self.stats[idx].synthesized += 1;
                            let key =
                                format!("{}-{}", def.key, self.stats[idx].synthesized);
                            let reference = Value::String(key.clone());
                            (key, reference)
                        }
                        MissingId::Hash => {
                            self.stats[idx].synthesized += 1;
                            let key = fnv1a_hex(&Value::Object(obj.clone()));
                            let reference = Value::String(key.clone());
                            (key, reference)
                        }
                    },
                };

                if self.tables[idx].contains_key(&key) {
                    if self.conflict == Conflict::Error {
                        return Err(format!(
                            "two \"{}\" entities share the id \"{key}\" (second one at {trail}); choose merge, replace, or keep first to allow it",
                            def.key
                        ));
                    }
                    self.stats[idx].merged += 1;
                    let existing = self.tables[idx].get_mut(&key).expect("checked above");
                    match self.conflict {
                        // Shallow merge: later keys win, earlier-only keys survive.
                        Conflict::Merge => {
                            if let Some(dst) = existing.as_object_mut() {
                                for (k, v) in obj {
                                    dst.insert(k, v);
                                }
                            }
                        }
                        Conflict::Replace => *existing = Value::Object(obj),
                        Conflict::KeepFirst | Conflict::Error => {}
                    }
                } else {
                    self.total += 1;
                    if self.total > MAX_ENTITIES {
                        return Err(format!("document holds more than {MAX_ENTITIES} entities"));
                    }
                    self.tables[idx].insert(key, Value::Object(obj));
                }
                Ok(reference)
            }
        }
    }
}

/// An id must be a scalar. Returns (table key, reference value); the reference
/// keeps the original JSON type, the table key is its string form.
fn scalar_id(v: &Value) -> Option<(String, Value)> {
    match v {
        Value::String(s) if !s.is_empty() => Some((s.clone(), v.clone())),
        Value::Number(n) => Some((n.to_string(), v.clone())),
        Value::Bool(b) => Some((b.to_string(), v.clone())),
        _ => None,
    }
}

/// Walk a dotted field path inside an entity object. Numeric segments index
/// into arrays so `authors.0.id` works.
fn path_mut<'v>(obj: &'v mut Map<String, Value>, segs: &[String]) -> Option<&'v mut Value> {
    let (first, rest) = segs.split_first()?;
    let mut cur = obj.get_mut(first.as_str())?;
    for s in rest {
        cur = match cur {
            Value::Object(m) => m.get_mut(s.as_str())?,
            Value::Array(a) => a.get_mut(s.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}

/// FNV-1a 64 over the entity's compact JSON — a stable content id for records
/// that have no id field of their own.
fn fnv1a_hex(v: &Value) -> String {
    let text = serde_json::to_string(v).unwrap_or_default();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Navigate the `path` parameter into the parsed document.
fn take_path<'v>(doc: &'v Value, path: &str) -> Result<&'v Value, String> {
    let cleaned = path
        .trim()
        .trim_start_matches('$')
        .replace(['[', '"', '\''], ".")
        .replace(']', "");
    let mut cur = doc;
    let mut walked = String::from("$");
    for seg in cleaned.split('.').filter(|s| !s.trim().is_empty()) {
        let seg = seg.trim();
        walked.push('.');
        walked.push_str(seg);
        cur = match cur {
            Value::Object(m) => m
                .get(seg)
                .ok_or_else(|| format!("path {walked} is not in the document"))?,
            Value::Array(a) => {
                let i: usize = seg
                    .parse()
                    .map_err(|_| format!("path {walked} indexes an array with \"{seg}\""))?;
                a.get(i)
                    .ok_or_else(|| format!("path {walked} is past the end of the array"))?
            }
            _ => return Err(format!("path {walked} runs past a scalar value")),
        };
    }
    Ok(cur)
}

// ------------------------------------------------------------------- api ----

/// Normalize `json` into entity tables keyed by id.
///
/// * `schema` — entity definitions, JSON object form or shorthand lines.
/// * `root` — the entity type the document (or each item of a top-level array) is.
/// * `path` — optional dotted path to the payload inside the document.
/// * `id_field` — id field name(s); one name, a comma-separated fallback list,
///   or a JSON object of entity -> field name(s).
/// * `on_missing_id` — `error` | `index` | `hash` | `keep`.
/// * `on_conflict` — `merge` | `replace` | `keep_first` | `error`.
/// * `output` — `normalized` | `entities` | `result` | `report`.
#[allow(clippy::too_many_arguments)]
pub fn normalize(
    json: &str,
    schema: &str,
    root: &str,
    path: &str,
    id_field: &str,
    on_missing_id: &str,
    on_conflict: &str,
    output: &str,
    pretty: bool,
    indent: usize,
) -> Result<String, String> {
    if json.trim().is_empty() {
        return Err("no JSON given: paste the document to normalize".into());
    }
    if json.len() > MAX_JSON_BYTES {
        return Err(format!(
            "document is {} bytes; the maximum is {MAX_JSON_BYTES}",
            json.len()
        ));
    }
    let missing = parse_enum(
        on_missing_id,
        "missing-id behavior",
        &[
            ("error", MissingId::Error),
            ("index", MissingId::Index),
            ("hash", MissingId::Hash),
            ("keep", MissingId::Keep),
        ],
    )?;
    let conflict = parse_enum(
        on_conflict,
        "duplicate-id behavior",
        &[
            ("merge", Conflict::Merge),
            ("replace", Conflict::Replace),
            ("keep_first", Conflict::KeepFirst),
            ("error", Conflict::Error),
        ],
    )?;
    let output = parse_enum(
        output,
        "output",
        &[
            ("normalized", Output::Normalized),
            ("entities", Output::Entities),
            ("result", Output::Result),
            ("report", Output::Report),
        ],
    )?;

    let mut schema = parse_schema(schema)?;
    apply_id_fields(&mut schema, id_field)?;

    let root = root.trim();
    if root.is_empty() {
        return Err(format!(
            "no root entity given: name the entity the document holds, one of: {}",
            schema
                .entities
                .iter()
                .map(|e| e.key.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let root_idx = schema.index_of(root).ok_or_else(|| {
        format!(
            "root entity \"{root}\" is not in the schema; declared entities: {}",
            schema
                .entities
                .iter()
                .map(|e| e.key.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;

    let doc: Value = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    let payload = take_path(&doc, path)?.clone();

    let mut norm = Normalizer::new(&schema, missing, conflict);
    let result = match payload {
        Value::Array(items) => {
            let mut ids = Vec::with_capacity(items.len());
            for (i, item) in items.into_iter().enumerate() {
                ids.push(norm.entity(&schema, root_idx, item, 0, &format!("$[{i}]"))?);
            }
            Value::Array(ids)
        }
        Value::Null => {
            return Err(
                "the document (or the value at the given path) is null; expected an object or an array of objects".into(),
            )
        }
        other => norm.entity(&schema, root_idx, other, 0, "$")?,
    };

    let mut entities = Map::new();
    for (i, def) in schema.entities.iter().enumerate() {
        entities.insert(
            def.key.clone(),
            Value::Object(std::mem::take(&mut norm.tables[i])),
        );
    }

    let value = match output {
        Output::Normalized => {
            let mut root_obj = Map::new();
            root_obj.insert("entities".into(), Value::Object(entities));
            root_obj.insert("result".into(), result);
            Value::Object(root_obj)
        }
        Output::Entities => Value::Object(entities),
        Output::Result => result,
        Output::Report => {
            return Ok(report(&schema, &entities, &result, root, path, &norm.stats))
        }
    };

    if pretty {
        let indent = indent.min(8);
        let pad = vec![b' '; indent];
        let mut buf = Vec::new();
        let formatter = serde_json::ser::PrettyFormatter::with_indent(&pad);
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
        serde::Serialize::serialize(&value, &mut ser)
            .map_err(|e| format!("could not serialize the result: {e}"))?;
        String::from_utf8(buf).map_err(|e| format!("could not serialize the result: {e}"))
    } else {
        serde_json::to_string(&value).map_err(|e| format!("could not serialize the result: {e}"))
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

fn report(
    schema: &Schema,
    entities: &Map<String, Value>,
    result: &Value,
    root: &str,
    path: &str,
    stats: &[Stats],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("Root entity: {root}\n"));
    out.push_str(&format!(
        "Payload path: {}\n",
        if path.trim().is_empty() {
            "(whole document)".to_string()
        } else {
            path.trim().to_string()
        }
    ));
    out.push_str(&match result {
        Value::Array(a) => format!("Result: array of {}\n", plural(a.len(), "id", "ids")),
        _ => "Result: 1 id\n".to_string(),
    });
    out.push_str("\nEntity tables\n");
    let mut synthesized = 0;
    let mut inline = 0;
    for (i, def) in schema.entities.iter().enumerate() {
        let stored = entities
            .get(&def.key)
            .and_then(|v| v.as_object())
            .map(|m| m.len())
            .unwrap_or(0);
        let s = &stats[i];
        synthesized += s.synthesized;
        inline += s.inline;
        out.push_str(&format!(
            "  {}: {} from {} ({} merged)\n",
            def.key,
            plural(stored, "entity", "entities"),
            plural(s.occurrences, "occurrence", "occurrences"),
            s.merged
        ));
    }
    out.push_str(&format!(
        "\nSynthesized ids: {synthesized}\nKept inline: {inline}\n"
    ));
    if entities.values().all(|v| v.as_object().is_none_or(|m| m.is_empty())) {
        out.push_str(
            "\nNothing was extracted. Check that the root entity matches the document and that the payload path points at the records.\n",
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r#"{
        "id": "123",
        "title": "My first post!",
        "author": {"id": "1", "name": "Paul"},
        "comments": [
            {"id": "324", "commenter": {"id": "2", "name": "Nicole"}},
            {"id": "325", "commenter": {"id": "1", "name": "Paul"}}
        ]
    }"#;

    const SCHEMA: &str = "articles: author -> users, comments -> [comments]\ncomments: commenter -> users\nusers:";

    fn run(json: &str, schema: &str, root: &str) -> Result<String, String> {
        normalize(json, schema, root, "", "", "", "", "", false, 2)
    }

    #[test]
    fn normalizes_nested_document_into_entity_tables() {
        let out = run(DOC, SCHEMA, "articles").unwrap();
        assert_eq!(
            out,
            r#"{"entities":{"articles":{"123":{"id":"123","title":"My first post!","author":"1","comments":["324","325"]}},"comments":{"324":{"id":"324","commenter":"2"},"325":{"id":"325","commenter":"1"}},"users":{"1":{"id":"1","name":"Paul"},"2":{"id":"2","name":"Nicole"}}},"result":"123"}"#
        );
    }

    #[test]
    fn json_schema_form_matches_shorthand_form() {
        let json_form = r#"{"articles":{"author":"users","comments":["comments"]},"comments":{"commenter":"users"},"users":{}}"#;
        assert_eq!(
            run(DOC, json_form, "articles").unwrap(),
            run(DOC, SCHEMA, "articles").unwrap()
        );
    }

    #[test]
    fn top_level_array_yields_an_array_result() {
        let doc = r#"[{"id":1,"author":{"id":9,"name":"Ada"}},{"id":2,"author":{"id":9,"name":"Ada L"}}]"#;
        let out = run(doc, "posts: author -> users\nusers:", "posts").unwrap();
        assert_eq!(
            out,
            r#"{"entities":{"posts":{"1":{"id":1,"author":9},"2":{"id":2,"author":9}},"users":{"9":{"id":9,"name":"Ada L"}}},"result":[1,2]}"#
        );
    }

    #[test]
    fn numeric_ids_keep_their_json_type_in_references() {
        let out = normalize(
            r#"{"id":7,"author":{"id":9}}"#,
            "posts: author -> users\nusers:",
            "posts",
            "",
            "",
            "",
            "",
            "result",
            false,
            2,
        )
        .unwrap();
        assert_eq!(out, "7");
    }

    #[test]
    fn payload_path_selects_the_records() {
        let doc = r#"{"meta":{"page":1},"data":{"items":[{"id":"a"},{"id":"b"}]}}"#;
        let out = normalize(
            doc, "items:", "items", "data.items", "", "", "", "entities", false, 2,
        )
        .unwrap();
        assert_eq!(out, r#"{"items":{"a":{"id":"a"},"b":{"id":"b"}}}"#);
    }

    #[test]
    fn custom_id_field_per_entity() {
        let doc = r#"{"id_str":"123","user":{"id_str":"456","name":"Jimmy"}}"#;
        let out = normalize(
            doc,
            "tweets: user -> users\nusers:",
            "tweets",
            "",
            r#"{"*":"id_str"}"#,
            "",
            "",
            "",
            false,
            2,
        )
        .unwrap();
        assert_eq!(
            out,
            r#"{"entities":{"tweets":{"123":{"id_str":"123","user":"456"}},"users":{"456":{"id_str":"456","name":"Jimmy"}}},"result":"123"}"#
        );
    }

    #[test]
    fn id_field_falls_back_through_a_comma_list() {
        let doc = r#"[{"_id":"a","v":1},{"uuid":"b","v":2}]"#;
        let out = normalize(
            doc, "rows:", "rows", "", "id,_id,uuid", "", "", "entities", false, 2,
        )
        .unwrap();
        assert_eq!(out, r#"{"rows":{"a":{"_id":"a","v":1},"b":{"uuid":"b","v":2}}}"#);
    }

    #[test]
    fn dotted_relation_paths_reach_nested_fields() {
        let doc = r#"{"id":1,"meta":{"author":{"id":5,"name":"Ada"}}}"#;
        let out = run(doc, "posts: meta.author -> users\nusers:", "posts").unwrap();
        assert_eq!(
            out,
            r#"{"entities":{"posts":{"1":{"id":1,"meta":{"author":5}}},"users":{"5":{"id":5,"name":"Ada"}}},"result":1}"#
        );
    }

    #[test]
    fn duplicate_ids_shallow_merge_by_default() {
        let doc = r#"[{"id":1,"a":"first"},{"id":1,"b":"second"}]"#;
        let out = normalize(doc, "rows:", "rows", "", "", "", "", "entities", false, 2).unwrap();
        assert_eq!(out, r#"{"rows":{"1":{"id":1,"a":"first","b":"second"}}}"#);
    }

    #[test]
    fn duplicate_ids_can_replace_or_keep_first() {
        let doc = r#"[{"id":1,"a":"first"},{"id":1,"b":"second"}]"#;
        let replace =
            normalize(doc, "rows:", "rows", "", "", "", "replace", "entities", false, 2).unwrap();
        assert_eq!(replace, r#"{"rows":{"1":{"id":1,"b":"second"}}}"#);
        let keep =
            normalize(doc, "rows:", "rows", "", "", "", "keep_first", "entities", false, 2)
                .unwrap();
        assert_eq!(keep, r#"{"rows":{"1":{"id":1,"a":"first"}}}"#);
    }

    #[test]
    fn missing_ids_can_be_indexed_hashed_or_kept() {
        let doc = r#"{"id":1,"author":{"name":"Ada"}}"#;
        let schema = "posts: author -> users\nusers:";
        let indexed =
            normalize(doc, schema, "posts", "", "", "index", "", "entities", false, 2).unwrap();
        assert_eq!(
            indexed,
            r#"{"posts":{"1":{"id":1,"author":"users-1"}},"users":{"users-1":{"name":"Ada"}}}"#
        );
        let kept =
            normalize(doc, schema, "posts", "", "", "keep", "", "entities", false, 2).unwrap();
        assert_eq!(
            kept,
            r#"{"posts":{"1":{"id":1,"author":{"name":"Ada"}}},"users":{}}"#
        );
        let hashed =
            normalize(doc, schema, "posts", "", "", "hash", "", "entities", false, 2).unwrap();
        assert!(
            hashed.contains(r#""author":"#),
            "hashed output keeps a reference: {hashed}"
        );
        // Same content hashes to the same id, so identical records collapse.
        let twice = normalize(
            r#"[{"id":1,"author":{"name":"Ada"}},{"id":2,"author":{"name":"Ada"}}]"#,
            schema,
            "posts",
            "",
            "",
            "hash",
            "",
            "entities",
            false,
            2,
        )
        .unwrap();
        let users = serde_json::from_str::<Value>(&twice).unwrap()["users"]
            .as_object()
            .unwrap()
            .len();
        assert_eq!(users, 1, "identical id-less records share a content id");
    }

    #[test]
    fn already_normalized_input_is_left_alone() {
        let once = run(DOC, SCHEMA, "articles").unwrap();
        let entities: Value = serde_json::from_str(&once).unwrap();
        let article = entities["entities"]["articles"]["123"].to_string();
        let twice = run(&article, SCHEMA, "articles").unwrap();
        let re: Value = serde_json::from_str(&twice).unwrap();
        assert_eq!(re["entities"]["articles"]["123"].to_string(), article);
        assert!(re["entities"]["users"].as_object().unwrap().is_empty());
    }

    #[test]
    fn pretty_output_indents() {
        let out = normalize(
            r#"{"id":1}"#, "rows:", "rows", "", "", "", "", "entities", true, 2,
        )
        .unwrap();
        assert_eq!(out, "{\n  \"rows\": {\n    \"1\": {\n      \"id\": 1\n    }\n  }\n}");
    }

    #[test]
    fn report_lists_every_table() {
        let out =
            normalize(DOC, SCHEMA, "articles", "", "", "", "", "report", false, 2).unwrap();
        assert!(out.contains("Root entity: articles"), "{out}");
        assert!(out.contains("articles: 1 entity from 1 occurrence (0 merged)"), "{out}");
        assert!(out.contains("users: 2 entities from 3 occurrences (1 merged)"), "{out}");
        assert!(out.contains("Synthesized ids: 0"), "{out}");
    }

    #[test]
    fn invalid_json_is_an_error() {
        let err = run("{not json", SCHEMA, "articles").unwrap_err();
        assert!(err.starts_with("invalid JSON:"), "{err}");
    }

    #[test]
    fn missing_id_is_an_error_by_default() {
        let err = run(r#"{"title":"x"}"#, "articles:", "articles").unwrap_err();
        assert!(err.contains("has no \"id\" field"), "{err}");
    }

    #[test]
    fn unknown_relation_target_is_an_error() {
        let err = run(DOC, "articles: author -> people", "articles").unwrap_err();
        assert!(err.contains("unknown entity \"people\""), "{err}");
    }

    #[test]
    fn unknown_root_entity_is_an_error() {
        let err = run(DOC, SCHEMA, "posts").unwrap_err();
        assert!(err.contains("root entity \"posts\" is not in the schema"), "{err}");
    }

    #[test]
    fn duplicate_id_can_be_an_error() {
        let doc = r#"[{"id":1},{"id":1}]"#;
        let err = normalize(doc, "rows:", "rows", "", "", "", "error", "", false, 2).unwrap_err();
        assert!(err.contains("share the id \"1\""), "{err}");
    }

    #[test]
    fn unknown_enum_value_is_an_error() {
        let err = normalize(DOC, SCHEMA, "articles", "", "", "", "", "table", false, 2)
            .unwrap_err();
        assert!(err.contains("unknown output \"table\""), "{err}");
        let err = normalize(DOC, SCHEMA, "articles", "", "", "maybe", "", "", false, 2)
            .unwrap_err();
        assert!(err.contains("unknown missing-id behavior \"maybe\""), "{err}");
    }

    #[test]
    fn bad_path_is_an_error() {
        let err = normalize(DOC, SCHEMA, "articles", "data.items", "", "", "", "", false, 2)
            .unwrap_err();
        assert!(err.contains("path $.data is not in the document"), "{err}");
    }

    #[test]
    fn empty_inputs_are_errors() {
        assert!(run("", SCHEMA, "articles").unwrap_err().contains("no JSON given"));
        assert!(run(DOC, "", "articles").unwrap_err().contains("schema is empty"));
        assert!(run(DOC, SCHEMA, "").unwrap_err().contains("no root entity given"));
    }

    #[test]
    fn array_where_a_single_entity_was_declared_is_lenient() {
        let doc = r#"{"id":1,"author":[{"id":2},{"id":3}]}"#;
        let out = normalize(
            doc,
            "posts: author -> users\nusers:",
            "posts",
            "",
            "",
            "",
            "",
            "entities",
            false,
            2,
        )
        .unwrap();
        assert_eq!(
            out,
            r#"{"posts":{"1":{"id":1,"author":[2,3]}},"users":{"2":{"id":2},"3":{"id":3}}}"#
        );
    }

    #[test]
    fn oversized_document_is_rejected() {
        let big = format!("{{\"id\":1,\"pad\":\"{}\"}}", "x".repeat(MAX_JSON_BYTES));
        let err = run(&big, "rows:", "rows").unwrap_err();
        assert!(err.contains("the maximum is"), "{err}");
    }
}
