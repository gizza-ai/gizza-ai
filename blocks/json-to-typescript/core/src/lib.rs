//! gizza-ai/json-to-typescript core — infer TypeScript interfaces from a JSON
//! sample. No wafer/wasm-bindgen deps (serde_json only). Merges array elements so
//! keys missing in some become optional (`?`) and differing types become unions.

use std::collections::BTreeMap;

use serde_json::Value;

/// A structurally-merged type node.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    /// Set of primitive TS type names (e.g. {"string", "null"}).
    Prim(std::collections::BTreeSet<&'static str>),
    Arr(Box<Node>),
    Obj(BTreeMap<String, Field>),
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Field {
    node: Node,
    optional: bool,
}

fn prim(name: &'static str) -> Node {
    let mut s = std::collections::BTreeSet::new();
    s.insert(name);
    Node::Prim(s)
}

/// Build a merged Node from a single JSON value.
fn from_value(v: &Value) -> Node {
    match v {
        Value::Null => prim("null"),
        Value::Bool(_) => prim("boolean"),
        Value::Number(_) => prim("number"),
        Value::String(_) => prim("string"),
        Value::Array(items) => {
            let mut inner = Node::Any;
            for it in items {
                inner = merge(inner, from_value(it));
            }
            Node::Arr(Box::new(inner))
        }
        Value::Object(map) => {
            let mut fields = BTreeMap::new();
            for (k, val) in map {
                fields.insert(k.clone(), Field { node: from_value(val), optional: false });
            }
            Node::Obj(fields)
        }
    }
}

/// Unify two nodes.
fn merge(a: Node, b: Node) -> Node {
    match (a, b) {
        (Node::Any, x) | (x, Node::Any) => x,
        (Node::Prim(mut s1), Node::Prim(s2)) => {
            s1.extend(s2);
            Node::Prim(s1)
        }
        (Node::Arr(a), Node::Arr(b)) => Node::Arr(Box::new(merge(*a, *b))),
        (Node::Obj(m1), Node::Obj(m2)) => {
            let mut out: BTreeMap<String, Field> = BTreeMap::new();
            for (k, f1) in &m1 {
                match m2.get(k) {
                    Some(f2) => out.insert(
                        k.clone(),
                        Field {
                            node: merge(f1.node.clone(), f2.node.clone()),
                            optional: f1.optional || f2.optional,
                        },
                    ),
                    None => out.insert(k.clone(), Field { node: f1.node.clone(), optional: true }),
                };
            }
            for (k, f2) in &m2 {
                if !m1.contains_key(k) {
                    out.insert(k.clone(), Field { node: f2.node.clone(), optional: true });
                }
            }
            Node::Obj(out)
        }
        // Incompatible shapes (e.g. object vs string) → `any`.
        _ => Node::Any,
    }
}

fn pascal(s: &str) -> String {
    let mut out = String::new();
    let mut up = true;
    for c in s.chars() {
        if c.is_alphanumeric() {
            if up {
                out.extend(c.to_uppercase());
                up = false;
            } else {
                out.push(c);
            }
        } else {
            up = true;
        }
    }
    if out.is_empty() || out.chars().next().unwrap().is_numeric() {
        out.insert(0, 'T');
    }
    out
}

fn singularize(s: &str) -> String {
    if let Some(stripped) = s.strip_suffix("ies") {
        format!("{stripped}y")
    } else if s.len() > 1 && s.ends_with('s') && !s.ends_with("ss") {
        s[..s.len() - 1].to_string()
    } else {
        format!("{s}Item")
    }
}

fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars().next().map(|c| c.is_ascii_alphabetic() || c == '_' || c == '$').unwrap_or(false)
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

struct Gen {
    /// Emitted interfaces in registration order: (name, body).
    interfaces: Vec<(String, String)>,
    names: std::collections::HashSet<String>,
    export: bool,
}

impl Gen {
    fn unique(&mut self, hint: &str) -> String {
        let base = if hint.is_empty() { "T".to_string() } else { hint.to_string() };
        if self.names.insert(base.clone()) {
            return base;
        }
        let mut i = 2;
        loop {
            let candidate = format!("{base}{i}");
            if self.names.insert(candidate.clone()) {
                return candidate;
            }
            i += 1;
        }
    }

    /// Emit a TS type expression for `node`, registering interfaces as needed.
    fn ty(&mut self, node: &Node, hint: &str) -> String {
        match node {
            Node::Any => "any".to_string(),
            Node::Prim(set) => set.iter().copied().collect::<Vec<_>>().join(" | "),
            Node::Arr(inner) => {
                let elem = self.ty(inner, &singularize(hint));
                if elem.contains(' ') {
                    format!("({elem})[]")
                } else {
                    format!("{elem}[]")
                }
            }
            Node::Obj(fields) => {
                let name = self.unique(hint);
                // Reserve the name immediately (already inserted by unique); build body.
                let mut body = String::new();
                for (k, f) in fields {
                    let key = if is_ident(k) { k.clone() } else { format!("\"{}\"", k.replace('"', "\\\"")) };
                    let opt = if f.optional { "?" } else { "" };
                    // child interface hint = PascalCase of the key
                    let child_hint = pascal(k);
                    let ty = self.ty(&f.node, &child_hint);
                    body.push_str(&format!("  {key}{opt}: {ty};\n"));
                }
                self.interfaces.push((name.clone(), body));
                name
            }
        }
    }

    fn render(&self) -> String {
        let kw = if self.export { "export interface" } else { "interface" };
        let mut out = String::new();
        for (name, body) in &self.interfaces {
            out.push_str(&format!("{kw} {name} {{\n{body}}}\n\n"));
        }
        out
    }
}

/// Infer TypeScript from a JSON string. `root_name` names the top-level type;
/// `export` prefixes declarations with `export`.
pub fn infer(json: &str, root_name: &str, export: bool) -> Result<String, String> {
    let value: Value = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    let root_name = if root_name.trim().is_empty() { "Root".to_string() } else { pascal(root_name) };
    let node = from_value(&value);

    let mut gen = Gen { interfaces: Vec::new(), names: std::collections::HashSet::new(), export };
    let top = gen.ty(&node, &root_name);

    let mut out = gen.render();
    // If the root wasn't an object that consumed `root_name`, add a type alias.
    if top != root_name {
        let kw = if export { "export type" } else { "type" };
        out.push_str(&format!("{kw} {root_name} = {top};\n"));
    }
    Ok(out.trim_end().to_string() + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_object() {
        let ts = infer(r#"{"name":"Ada","age":30,"active":true}"#, "User", true).unwrap();
        assert!(ts.contains("export interface User {"), "got:\n{ts}");
        assert!(ts.contains("name: string;"));
        assert!(ts.contains("age: number;"));
        assert!(ts.contains("active: boolean;"));
    }

    #[test]
    fn nested_object_gets_own_interface() {
        let ts = infer(r#"{"user":{"id":1}}"#, "Root", true).unwrap();
        assert!(ts.contains("export interface User {"), "got:\n{ts}");
        assert!(ts.contains("id: number;"));
        assert!(ts.contains("user: User;"));
    }

    #[test]
    fn array_optional_and_union() {
        let ts = infer(r#"{"items":[{"a":1},{"a":2,"b":"x"}]}"#, "Root", true).unwrap();
        // element interface from singular of "items" => "Item"
        assert!(ts.contains("interface Item {"), "got:\n{ts}");
        assert!(ts.contains("a: number;"));
        assert!(ts.contains("b?: string;"), "b should be optional: {ts}");
        assert!(ts.contains("items: Item[];"));
    }

    #[test]
    fn primitive_union_in_array() {
        let ts = infer(r#"{"mixed":[1,"two",true]}"#, "Root", false).unwrap();
        assert!(ts.contains("mixed: (boolean | number | string)[];"), "got:\n{ts}");
    }

    #[test]
    fn nullable_field() {
        let ts = infer(r#"{"x":null}"#, "Root", false).unwrap();
        assert!(ts.contains("x: null;"), "got:\n{ts}");
    }

    #[test]
    fn root_array_uses_type_alias() {
        let ts = infer(r#"[{"id":1}]"#, "Row", true).unwrap();
        assert!(ts.contains("interface Row {") || ts.contains("interface RowItem {"), "got:\n{ts}");
        assert!(ts.contains("export type Row ="), "got:\n{ts}");
        assert!(ts.contains("[];"));
    }

    #[test]
    fn non_identifier_key_is_quoted() {
        let ts = infer(r#"{"first-name":"Ada"}"#, "Root", false).unwrap();
        assert!(ts.contains("\"first-name\": string;"), "got:\n{ts}");
    }

    #[test]
    fn invalid_json_errors() {
        assert!(infer("{not json}", "Root", true).is_err());
    }
}
