//! gizza-ai/jq-query core — run a `jq` program against a JSON document using
//! `jaq` (a pure-Rust, `no_std`+alloc jq implementation). No wafer/wasm-bindgen
//! deps and no WASI/host deps, so it runs in the gizza wafer runtime.
//!
//! Returns each produced value as a separate output string (jq filters can yield
//! zero, one, or many results). `pretty` controls compact vs indented JSON.

use jaq_interpret::{Ctx, FilterT, ParseCtx, RcIter, Val};
use serde_json::Value;

/// Run `program` over the JSON in `input`. Returns the produced values as
/// serialized JSON strings (one per output). Errors on invalid JSON, a jq parse
/// or compile error, or a runtime error.
pub fn run_jq(program: &str, input: &str, pretty: bool) -> Result<Vec<String>, String> {
    if program.trim().is_empty() {
        return Err("jq program is empty".into());
    }
    let value: Value = serde_json::from_str(input).map_err(|e| format!("invalid JSON input: {e}"))?;

    // Context with core natives + the jq standard library (map, select, …).
    let mut defs = ParseCtx::new(Vec::new());
    defs.insert_natives(jaq_core::core());
    defs.insert_defs(jaq_std::std());

    let (parsed, errs) = jaq_parse::parse(program, jaq_parse::main());
    if !errs.is_empty() {
        let msg = errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ");
        return Err(format!("jq parse error: {msg}"));
    }
    let parsed = parsed.ok_or_else(|| "jq program produced no filter".to_string())?;

    let filter = defs.compile(parsed);
    if !defs.errs.is_empty() {
        let msg = defs
            .errs
            .iter()
            .map(|(e, _span)| e.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!("jq compile error: {msg}"));
    }

    let inputs = RcIter::new(core::iter::empty());
    let out = filter.run((Ctx::new([], &inputs), Val::from(value)));

    let mut results = Vec::new();
    for item in out {
        let v = item.map_err(|e| format!("jq runtime error: {e}"))?;
        let json = Value::from(v);
        let s = if pretty {
            serde_json::to_string_pretty(&json)
        } else {
            serde_json::to_string(&json)
        }
        .map_err(|e| format!("serialize error: {e}"))?;
        results.push(s);
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity() {
        assert_eq!(run_jq(".", "42", false).unwrap(), vec!["42"]);
    }

    #[test]
    fn field_access() {
        assert_eq!(run_jq(".name", r#"{"name":"Ada","age":36}"#, false).unwrap(), vec![r#""Ada""#]);
    }

    #[test]
    fn iterate_array_yields_multiple() {
        let out = run_jq(".[]", "[1,2,3]", false).unwrap();
        assert_eq!(out, vec!["1", "2", "3"]);
    }

    #[test]
    fn std_library_map_select() {
        let out = run_jq("map(select(. % 2 == 0))", "[1,2,3,4,5,6]", false).unwrap();
        assert_eq!(out, vec!["[2,4,6]"]);
    }

    #[test]
    fn object_construction_and_pipe() {
        let out = run_jq(
            r#".users[] | {n: .name, adult: (.age >= 18)}"#,
            r#"{"users":[{"name":"Al","age":20},{"name":"Bo","age":10}]}"#,
            false,
        )
        .unwrap();
        // jaq emits object keys in sorted order.
        assert_eq!(out, vec![r#"{"adult":true,"n":"Al"}"#, r#"{"adult":false,"n":"Bo"}"#]);
    }

    #[test]
    fn pretty_print() {
        let out = run_jq(".", r#"{"a":1}"#, true).unwrap();
        assert_eq!(out, vec!["{\n  \"a\": 1\n}"]);
    }

    #[test]
    fn errors() {
        assert!(run_jq(".", "{not json}", false).is_err()); // bad JSON
        assert!(run_jq("", "1", false).is_err()); // empty program
        assert!(run_jq(". |", "1", false).is_err()); // parse error
        assert!(run_jq(".a + .b", r#"{"a":1,"b":"x"}"#, false).is_err()); // runtime type error
    }
}
