//! json-structure-analyzer core — pure compute, shared by the chat skill block and the web page.
//! Parses a JSON document and reports structural statistics: max nesting depth, node counts by
//! type, key frequency, per-path type distribution, array-length stats, byte sizes, and quality
//! warnings. No wafer/wasm-bindgen deps.

use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// Output shape selector.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Pretty-printed JSON report (default).
    Json,
    /// Human-readable plain-text report.
    Text,
}

pub struct Options {
    pub format: Format,
    /// Max entries to list in the key-frequency ranking (0 = all).
    pub top_keys: usize,
    /// Max entries to list in the per-path type distribution (0 = all).
    pub top_paths: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options { format: Format::Json, top_keys: 30, top_paths: 50 }
    }
}

#[derive(Serialize, Default)]
struct Counts {
    objects: u64,
    arrays: u64,
    strings: u64,
    numbers: u64,
    booleans: u64,
    nulls: u64,
    /// Every object-key occurrence across the whole document (recurring names counted each time).
    total_keys: u64,
    /// Distinct object-key names.
    unique_keys: u64,
    /// Every value node (containers + primitives), the root included.
    total_values: u64,
    /// Null values plus empty strings (`""`) plus empty objects/arrays.
    empty_values: u64,
}

#[derive(Serialize)]
struct SizeInfo {
    /// Byte length of the input exactly as pasted.
    bytes: usize,
    /// Byte length after re-serializing with all insignificant whitespace removed.
    minified_bytes: usize,
    /// Percent that minifying would save (0 if the input is already compact or smaller).
    compression_potential_pct: u64,
}

#[derive(Serialize)]
struct KeyCount {
    key: String,
    count: u64,
}

#[derive(Serialize)]
struct PathTypes {
    /// Normalized path: `$` is the root, `.key` descends an object, `[]` descends an array
    /// (array indices are collapsed so every element of an array shares one path).
    path: String,
    /// Distinct JSON types seen at this path, sorted.
    types: Vec<String>,
    /// How many nodes were observed at this path.
    count: u64,
}

#[derive(Serialize)]
struct ArrayStats {
    count: u64,
    min_length: u64,
    max_length: u64,
    avg_length: f64,
    total_elements: u64,
}

#[derive(Serialize)]
struct Report {
    valid: bool,
    root_type: String,
    /// Deepest nesting level. The root value is depth 0; each step into an object or array
    /// adds 1 (so `{"a":1}` is depth 1 and `{"a":{"b":1}}` is depth 2).
    max_depth: u64,
    counts: Counts,
    size: SizeInfo,
    key_frequency: Vec<KeyCount>,
    /// True when the key-frequency list above was truncated by `top_keys`.
    key_frequency_truncated: bool,
    paths: Vec<PathTypes>,
    /// True when the per-path list above was truncated by `top_paths`.
    paths_truncated: bool,
    arrays: ArrayStats,
    warnings: Vec<String>,
}

/// Deep-nesting is flagged past this level (common convention across analyzers).
const DEEP_NESTING: u64 = 5;

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

struct Walker {
    counts: Counts,
    max_depth: u64,
    key_freq: BTreeMap<String, u64>,
    /// path -> (set of types via BTreeMap<type, ()>, count)
    paths: BTreeMap<String, (std::collections::BTreeSet<&'static str>, u64)>,
    array_lengths: Vec<u64>,
}

impl Walker {
    fn walk(&mut self, v: &Value, path: &str, depth: u64) {
        self.counts.total_values += 1;
        if depth > self.max_depth {
            self.max_depth = depth;
        }
        let ty = type_name(v);
        let entry = self.paths.entry(path.to_string()).or_insert_with(|| {
            (std::collections::BTreeSet::new(), 0)
        });
        entry.0.insert(ty);
        entry.1 += 1;

        match v {
            Value::Object(map) => {
                self.counts.objects += 1;
                if map.is_empty() {
                    self.counts.empty_values += 1;
                }
                for (k, child) in map {
                    self.counts.total_keys += 1;
                    *self.key_freq.entry(k.clone()).or_insert(0) += 1;
                    let child_path = format!("{path}.{k}");
                    self.walk(child, &child_path, depth + 1);
                }
            }
            Value::Array(arr) => {
                self.counts.arrays += 1;
                self.array_lengths.push(arr.len() as u64);
                if arr.is_empty() {
                    self.counts.empty_values += 1;
                }
                let child_path = format!("{path}[]");
                for child in arr {
                    self.walk(child, &child_path, depth + 1);
                }
            }
            Value::String(s) => {
                self.counts.strings += 1;
                if s.is_empty() {
                    self.counts.empty_values += 1;
                }
            }
            Value::Number(_) => self.counts.numbers += 1,
            Value::Bool(_) => self.counts.booleans += 1,
            Value::Null => {
                self.counts.nulls += 1;
                self.counts.empty_values += 1;
            }
        }
    }
}

fn build_report(input: &str, opts: &Options) -> Result<Report, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("no JSON provided — paste a JSON document to analyze".into());
    }
    let value: Value = serde_json::from_str(trimmed).map_err(|e| format!("invalid JSON: {e}"))?;

    let mut w = Walker {
        counts: Counts::default(),
        max_depth: 0,
        key_freq: BTreeMap::new(),
        paths: BTreeMap::new(),
        array_lengths: Vec::new(),
    };
    w.walk(&value, "$", 0);
    w.counts.unique_keys = w.key_freq.len() as u64;

    // Key frequency: sort by count desc, then key asc for stable output.
    let mut key_freq: Vec<KeyCount> = w
        .key_freq
        .into_iter()
        .map(|(key, count)| KeyCount { key, count })
        .collect();
    key_freq.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));
    let key_frequency_truncated = opts.top_keys != 0 && key_freq.len() > opts.top_keys;
    if opts.top_keys != 0 {
        key_freq.truncate(opts.top_keys);
    }

    // Paths: sort by count desc, then path asc.
    let mut paths: Vec<PathTypes> = w
        .paths
        .into_iter()
        .map(|(path, (types, count))| PathTypes {
            path,
            types: types.into_iter().map(|s| s.to_string()).collect(),
            count,
        })
        .collect();
    paths.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.path.cmp(&b.path)));
    let paths_truncated = opts.top_paths != 0 && paths.len() > opts.top_paths;
    if opts.top_paths != 0 {
        paths.truncate(opts.top_paths);
    }

    // Array stats.
    let arr_count = w.array_lengths.len() as u64;
    let total_elements: u64 = w.array_lengths.iter().sum();
    let arrays = ArrayStats {
        count: arr_count,
        min_length: w.array_lengths.iter().copied().min().unwrap_or(0),
        max_length: w.array_lengths.iter().copied().max().unwrap_or(0),
        avg_length: if arr_count == 0 {
            0.0
        } else {
            // Round to 2 decimals for a clean report.
            ((total_elements as f64 / arr_count as f64) * 100.0).round() / 100.0
        },
        total_elements,
    };

    // Size.
    let bytes = input.len();
    let minified = serde_json::to_string(&value).unwrap_or_default();
    let minified_bytes = minified.len();
    let compression_potential_pct = if bytes > 0 && minified_bytes < bytes {
        (((bytes - minified_bytes) as f64 / bytes as f64) * 100.0).round() as u64
    } else {
        0
    };
    let size = SizeInfo { bytes, minified_bytes, compression_potential_pct };

    // Warnings.
    let mut warnings = Vec::new();
    if w.max_depth > DEEP_NESTING {
        warnings.push(format!(
            "Deep nesting: {} levels (> {} may be hard to work with)",
            w.max_depth, DEEP_NESTING
        ));
    }
    let recurring: usize = key_freq.iter().filter(|k| k.count > 1).count();
    if recurring > 0 {
        warnings.push(format!(
            "{recurring} key name(s) recur across the document (see key_frequency)"
        ));
    }
    if w.counts.empty_values > 0 {
        warnings.push(format!(
            "{} empty value(s): nulls, empty strings, or empty objects/arrays",
            w.counts.empty_values
        ));
    }

    Ok(Report {
        valid: true,
        root_type: type_name(&value).to_string(),
        max_depth: w.max_depth,
        counts: w.counts,
        size,
        key_frequency: key_freq,
        key_frequency_truncated,
        paths,
        paths_truncated,
        arrays,
        warnings,
    })
}

fn render_text(r: &Report) -> String {
    let mut out = String::new();
    out.push_str("JSON Structure Analysis\n");
    out.push_str("=======================\n");
    out.push_str(&format!("Root type:     {}\n", r.root_type));
    out.push_str(&format!("Max depth:     {}\n", r.max_depth));
    out.push_str(&format!(
        "Size:          {} bytes ({} minified, {}% smaller)\n",
        r.size.bytes, r.size.minified_bytes, r.size.compression_potential_pct
    ));
    out.push('\n');

    out.push_str("Node counts\n");
    out.push_str(&format!("  objects:      {}\n", r.counts.objects));
    out.push_str(&format!("  arrays:       {}\n", r.counts.arrays));
    out.push_str(&format!("  strings:      {}\n", r.counts.strings));
    out.push_str(&format!("  numbers:      {}\n", r.counts.numbers));
    out.push_str(&format!("  booleans:     {}\n", r.counts.booleans));
    out.push_str(&format!("  nulls:        {}\n", r.counts.nulls));
    out.push_str(&format!("  total values: {}\n", r.counts.total_values));
    out.push_str(&format!("  total keys:   {}\n", r.counts.total_keys));
    out.push_str(&format!("  unique keys:  {}\n", r.counts.unique_keys));
    out.push_str(&format!("  empty values: {}\n", r.counts.empty_values));
    out.push('\n');

    out.push_str("Arrays\n");
    out.push_str(&format!("  count:        {}\n", r.arrays.count));
    out.push_str(&format!("  lengths:      min {}, max {}, avg {}\n", r.arrays.min_length, r.arrays.max_length, r.arrays.avg_length));
    out.push_str(&format!("  elements:     {}\n", r.arrays.total_elements));
    out.push('\n');

    out.push_str(&format!("Key frequency (top {})\n", r.key_frequency.len()));
    if r.key_frequency.is_empty() {
        out.push_str("  (no object keys)\n");
    } else {
        for k in &r.key_frequency {
            out.push_str(&format!("  {:>6}  {}\n", k.count, k.key));
        }
    }
    if r.key_frequency_truncated {
        out.push_str("  … (truncated)\n");
    }
    out.push('\n');

    out.push_str(&format!("Type distribution by path ({} shown)\n", r.paths.len()));
    for p in &r.paths {
        out.push_str(&format!("  {:>6}  {}  [{}]\n", p.count, p.path, p.types.join(", ")));
    }
    if r.paths_truncated {
        out.push_str("  … (truncated)\n");
    }

    if !r.warnings.is_empty() {
        out.push('\n');
        out.push_str("Warnings\n");
        for w in &r.warnings {
            out.push_str(&format!("  • {w}\n"));
        }
    }

    out
}

/// Analyze `input` (a JSON document) and return a structural report per `opts`.
pub fn analyze(input: &str, opts: &Options) -> Result<String, String> {
    let report = build_report(input, opts)?;
    match opts.format {
        Format::Json => {
            serde_json::to_string_pretty(&report).map_err(|e| format!("serialization error: {e}"))
        }
        Format::Text => Ok(render_text(&report)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json_report(input: &str) -> Value {
        let s = analyze(input, &Options::default()).unwrap();
        serde_json::from_str(&s).unwrap()
    }

    #[test]
    fn analyzes_nested_object() {
        let r = json_report(r#"{"user":{"id":1,"tags":["a","b"]},"active":true}"#);
        assert_eq!(r["root_type"], "object");
        assert_eq!(r["max_depth"], 3); // $ -> .user -> .tags -> []
        assert_eq!(r["counts"]["objects"], 2);
        assert_eq!(r["counts"]["arrays"], 1);
        assert_eq!(r["counts"]["strings"], 2);
        assert_eq!(r["counts"]["numbers"], 1);
        assert_eq!(r["counts"]["booleans"], 1);
        assert_eq!(r["counts"]["total_keys"], 4); // user, id, tags, active
        assert_eq!(r["counts"]["unique_keys"], 4);
        assert_eq!(r["arrays"]["max_length"], 2);
    }

    #[test]
    fn key_frequency_counts_recurrence() {
        let r = json_report(r#"[{"id":1,"name":"a"},{"id":2,"name":"b"},{"id":3}]"#);
        // id appears 3x, name 2x.
        let kf = r["key_frequency"].as_array().unwrap();
        let id = kf.iter().find(|e| e["key"] == "id").unwrap();
        let name = kf.iter().find(|e| e["key"] == "name").unwrap();
        assert_eq!(id["count"], 3);
        assert_eq!(name["count"], 2);
        // sorted: id first (highest count).
        assert_eq!(kf[0]["key"], "id");
        assert_eq!(r["arrays"]["count"], 1);
        assert_eq!(r["arrays"]["max_length"], 3);
    }

    #[test]
    fn path_collapses_array_indices() {
        let r = json_report(r#"{"users":[{"name":"a"},{"name":"b"}]}"#);
        let paths = r["paths"].as_array().unwrap();
        // Every user's name shares "$.users[].name".
        let name_path = paths.iter().find(|p| p["path"] == "$.users[].name").unwrap();
        assert_eq!(name_path["count"], 2);
        assert_eq!(name_path["types"][0], "string");
    }

    #[test]
    fn mixed_types_at_path() {
        let r = json_report(r#"{"vals":[1,"two",null]}"#);
        let paths = r["paths"].as_array().unwrap();
        let vp = paths.iter().find(|p| p["path"] == "$.vals[]").unwrap();
        let types: Vec<String> = vp["types"].as_array().unwrap().iter().map(|t| t.as_str().unwrap().to_string()).collect();
        assert!(types.contains(&"number".to_string()));
        assert!(types.contains(&"string".to_string()));
        assert!(types.contains(&"null".to_string()));
    }

    #[test]
    fn empty_values_counted() {
        let r = json_report(r#"{"a":null,"b":"","c":[],"d":{}}"#);
        assert_eq!(r["counts"]["empty_values"], 4);
        assert_eq!(r["counts"]["nulls"], 1);
    }

    #[test]
    fn deep_nesting_warns() {
        let r = json_report(r#"{"a":{"b":{"c":{"d":{"e":{"f":1}}}}}}"#);
        assert_eq!(r["max_depth"], 6);
        let warnings = r["warnings"].as_array().unwrap();
        assert!(warnings.iter().any(|w| w.as_str().unwrap().contains("Deep nesting")));
    }

    #[test]
    fn scalar_root_depth_zero() {
        let r = json_report("42");
        assert_eq!(r["root_type"], "number");
        assert_eq!(r["max_depth"], 0);
        assert_eq!(r["counts"]["numbers"], 1);
    }

    #[test]
    fn top_keys_truncates() {
        let opts = Options { format: Format::Json, top_keys: 1, top_paths: 0 };
        let s = analyze(r#"{"aaa":1,"bbb":2,"ccc":3}"#, &opts).unwrap();
        let r: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(r["key_frequency"].as_array().unwrap().len(), 1);
        assert_eq!(r["key_frequency_truncated"], true);
    }

    #[test]
    fn text_format_renders() {
        let opts = Options { format: Format::Text, ..Default::default() };
        let out = analyze(r#"{"a":1}"#, &opts).unwrap();
        assert!(out.contains("JSON Structure Analysis"));
        assert!(out.contains("Max depth:"));
        assert!(out.contains("Key frequency"));
    }

    #[test]
    fn invalid_json_errors() {
        let err = analyze("{not valid}", &Options::default()).unwrap_err();
        assert!(err.contains("invalid JSON"));
    }

    #[test]
    fn empty_input_errors() {
        let err = analyze("   ", &Options::default()).unwrap_err();
        assert!(err.contains("no JSON"));
    }
}
