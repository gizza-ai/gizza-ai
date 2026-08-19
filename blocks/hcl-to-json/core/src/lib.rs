//! hcl-to-json core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Parses HashiCorp Configuration Language (HCL2 — Terraform `.tf`/`.tfvars`,
//! Packer, Nomad, Vault, Consul) and renders the equivalent JSON document.
//!
//! Representation rules (both directions of the `blocks` switch):
//! * An `attribute = value` line becomes a JSON property.
//! * A `block "label" "label" { … }` becomes nested objects — one level per
//!   label — with the block body as the innermost value.
//! * `blocks = "nested"` (default) writes a bare object for a block that occurs
//!   once and a JSON array when the same block header repeats.
//!   `blocks = "arrays"` always writes an array, so scripts get one stable shape.
//! * Expressions that JSON cannot represent (`var.x`, `upper(s)`, `a ? b : c`)
//!   are written as Terraform-style `"${…}"` interpolation strings, which is the
//!   same convention Terraform's own JSON syntax uses. `expressions = "simplify"`
//!   additionally folds constant sub-expressions (`1 + 2` → `3`).
//! * Comments are dropped — JSON has no comment syntax.

use std::collections::HashMap;

use hcl::eval::{Context, Evaluate};
use hcl::expr::ObjectKey;
use hcl::{Block, Body, Expression, Structure};
use serde_json::{Map, Value};

/// Largest accepted HCL input, in bytes. Big enough for any realistic single
/// Terraform module file; keeps the browser/WASM sandbox well inside its memory
/// budget.
pub const MAX_INPUT_BYTES: usize = 1_048_576;

/// Convert HCL text to JSON.
///
/// * `blocks` — `"nested"` (default) or `"arrays"`.
/// * `expressions` — `"template"` (default) or `"simplify"`.
/// * `sort_keys` — sort every object's keys alphabetically instead of keeping
///   the order they appear in the source.
/// * `pretty` — indent the JSON over multiple lines.
/// * `indent` — `"2"` (default), `"4"` or `"tab"`; ignored when `pretty` is false.
pub fn convert(
    input: &str,
    blocks: &str,
    expressions: &str,
    sort_keys: bool,
    pretty: bool,
    indent: &str,
) -> Result<String, String> {
    let always_array = match blocks.trim() {
        "" | "nested" => false,
        "arrays" => true,
        other => {
            return Err(format!(
                "unknown blocks mode '{other}': expected 'nested' or 'arrays'"
            ))
        }
    };
    let simplify = match expressions.trim() {
        "" | "template" => false,
        "simplify" => true,
        other => {
            return Err(format!(
                "unknown expressions mode '{other}': expected 'template' or 'simplify'"
            ))
        }
    };
    let indent_unit: &[u8] = match indent.trim() {
        "" | "2" => b"  ",
        "4" => b"    ",
        "tab" => b"\t",
        other => {
            return Err(format!(
                "unknown indent '{other}': expected '2', '4' or 'tab'"
            ))
        }
    };

    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "HCL input is {} bytes, which is over the {} byte limit — split the configuration into smaller files",
            input.len(),
            MAX_INPUT_BYTES
        ));
    }
    if input.trim().is_empty() {
        return Err("HCL input is empty — paste a Terraform/HCL configuration such as 'resource \"aws_instance\" \"web\" { ami = \"ami-123\" }'".to_string());
    }

    let body: Body = hcl::from_str(input).map_err(|e| {
        let msg = e.to_string();
        let msg = msg.trim();
        if msg.is_empty() {
            "could not parse the input as HCL".to_string()
        } else {
            msg.to_string()
        }
    })?;

    let mut value = body_to_json(&body, always_array, simplify)?;
    if sort_keys {
        sort_value(&mut value);
    }
    render(&value, pretty, indent_unit)
}

/// The block header path: the block identifier followed by each label, e.g.
/// `resource "aws_instance" "web"` → `["resource", "aws_instance", "web"]`.
fn block_path(block: &Block) -> Vec<String> {
    let mut path = Vec::with_capacity(1 + block.labels.len());
    path.push(block.identifier.as_str().to_string());
    for label in &block.labels {
        path.push(label.as_str().to_string());
    }
    path
}

fn body_to_json(body: &Body, always_array: bool, simplify: bool) -> Result<Value, String> {
    // A block header that appears more than once in the same body collapses into
    // a JSON array even in "nested" mode, so count the headers up front.
    let mut repeats: HashMap<Vec<String>, usize> = HashMap::new();
    for structure in body.iter() {
        if let Structure::Block(block) = structure {
            *repeats.entry(block_path(block)).or_insert(0) += 1;
        }
    }

    let mut map = Map::new();
    for structure in body.iter() {
        match structure {
            Structure::Attribute(attr) => {
                let key = attr.key.as_str();
                if map.contains_key(key) {
                    return Err(conflict(key));
                }
                map.insert(key.to_string(), expr_to_json(&attr.expr, simplify));
            }
            Structure::Block(block) => {
                let path = block_path(block);
                let as_array = always_array || repeats.get(&path).copied().unwrap_or(1) > 1;
                let child = body_to_json(&block.body, always_array, simplify)?;
                insert_block(&mut map, &path, child, as_array)?;
            }
        }
    }
    Ok(Value::Object(map))
}

fn conflict(name: &str) -> String {
    format!(
        "conflicting definitions for '{name}': the same name is used both as an attribute (or a block with a different number of labels) and as a block — rename one of them"
    )
}

fn insert_block(
    map: &mut Map<String, Value>,
    path: &[String],
    child: Value,
    as_array: bool,
) -> Result<(), String> {
    let (head, rest) = path.split_first().expect("block path is never empty");
    if rest.is_empty() {
        if as_array {
            match map.get_mut(head) {
                Some(Value::Array(items)) => items.push(child),
                Some(_) => return Err(conflict(head)),
                None => {
                    map.insert(head.clone(), Value::Array(vec![child]));
                }
            }
        } else if map.contains_key(head) {
            return Err(conflict(head));
        } else {
            map.insert(head.clone(), child);
        }
        return Ok(());
    }
    let entry = map
        .entry(head.clone())
        .or_insert_with(|| Value::Object(Map::new()));
    match entry {
        Value::Object(inner) => insert_block(inner, rest, child, as_array),
        _ => Err(conflict(head)),
    }
}

fn expr_to_json(expr: &Expression, simplify: bool) -> Value {
    if simplify {
        if let Ok(value) = expr.evaluate(&Context::new()) {
            return hcl_value_to_json(value);
        }
        // Whole-expression evaluation failed (an unknown variable or function is
        // in there somewhere) — keep folding the parts that ARE constant.
        match expr {
            Expression::Array(items) => {
                return Value::Array(items.iter().map(|e| expr_to_json(e, true)).collect())
            }
            Expression::Object(object) => {
                let mut map = Map::new();
                for (key, value) in object.iter() {
                    map.insert(object_key_string(key), expr_to_json(value, true));
                }
                return Value::Object(map);
            }
            Expression::Parenthesis(inner) => return expr_to_json(inner, true),
            _ => {}
        }
    }
    hcl_value_to_json(hcl::Value::from(expr.clone()))
}

fn object_key_string(key: &ObjectKey) -> String {
    match hcl::Value::from(key.clone()) {
        hcl::Value::String(s) => s,
        other => other.to_string(),
    }
}

fn hcl_value_to_json(value: hcl::Value) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

fn sort_value(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = std::mem::take(map).into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            for (_, v) in entries.iter_mut() {
                sort_value(v);
            }
            for (k, v) in entries {
                map.insert(k, v);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(sort_value),
        _ => {}
    }
}

fn render(value: &Value, pretty: bool, indent_unit: &[u8]) -> Result<String, String> {
    if !pretty {
        return serde_json::to_string(value).map_err(|e| format!("could not render JSON: {e}"));
    }
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(indent_unit);
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    serde::Serialize::serialize(value, &mut ser)
        .map_err(|e| format!("could not render JSON: {e}"))?;
    String::from_utf8(buf).map_err(|e| format!("could not render JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TF: &str = r#"
# provider pinning
terraform {
  required_version = ">= 1.5"
}

variable "region" {
  type    = string
  default = "us-east-1"
}

resource "aws_instance" "web" {
  ami           = "ami-0123456789"
  instance_type = "t3.micro"
  monitoring    = true
  count         = 2
  tags = {
    Name = "web"
    Env  = "prod"
  }
  ports = [80, 443]

  ebs_block_device {
    device_name = "/dev/sda"
    volume_size = 10
  }

  ebs_block_device {
    device_name = "/dev/sdb"
    volume_size = 20
  }
}
"#;

    #[test]
    fn converts_terraform_blocks_labels_and_attributes() {
        let out = convert(TF, "nested", "template", false, true, "2").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["terraform"]["required_version"], ">= 1.5");
        // `type = string` is a bare variable reference, so it round-trips as an
        // interpolation string rather than being lost.
        assert_eq!(v["variable"]["region"]["type"], "${string}");
        assert_eq!(v["variable"]["region"]["default"], "us-east-1");
        let web = &v["resource"]["aws_instance"]["web"];
        assert_eq!(web["ami"], "ami-0123456789");
        assert_eq!(web["monitoring"], true);
        assert_eq!(web["count"], 2);
        assert_eq!(web["tags"]["Env"], "prod");
        assert_eq!(web["ports"], serde_json::json!([80, 443]));
        // Repeated block header → JSON array, in source order.
        assert_eq!(web["ebs_block_device"].as_array().unwrap().len(), 2);
        assert_eq!(web["ebs_block_device"][1]["volume_size"], 20);
    }

    #[test]
    fn source_key_order_is_preserved_by_default() {
        let out = convert("zeta = 1\nalpha = 2\n", "nested", "template", false, false, "2").unwrap();
        assert_eq!(out, r#"{"zeta":1,"alpha":2}"#);
    }

    #[test]
    fn sort_keys_sorts_every_level() {
        let out = convert(
            "zeta = 1\nalpha = 2\n\nb \"x\" {\n  n = 1\n  m = 2\n}\n",
            "nested",
            "template",
            true,
            false,
            "2",
        )
        .unwrap();
        assert_eq!(out, r#"{"alpha":2,"b":{"x":{"m":2,"n":1}},"zeta":1}"#);
    }

    #[test]
    fn arrays_mode_wraps_every_block_body() {
        let out = convert(
            "resource \"aws_s3_bucket\" \"logs\" {\n  bucket = \"my-logs\"\n}\n",
            "arrays",
            "template",
            false,
            false,
            "2",
        )
        .unwrap();
        assert_eq!(
            out,
            r#"{"resource":{"aws_s3_bucket":{"logs":[{"bucket":"my-logs"}]}}}"#
        );
    }

    #[test]
    fn arrays_mode_appends_repeated_blocks() {
        let src = "provisioner \"local-exec\" {\n  command = \"a\"\n}\n\nprovisioner \"local-exec\" {\n  command = \"b\"\n}\n";
        let out = convert(src, "arrays", "template", false, false, "2").unwrap();
        assert_eq!(
            out,
            r#"{"provisioner":{"local-exec":[{"command":"a"},{"command":"b"}]}}"#
        );
    }

    #[test]
    fn unlabeled_block_repeats_collapse_to_an_array_in_nested_mode() {
        let out = convert(
            "locals {\n  a = 1\n}\n\nlocals {\n  b = 2\n}\n",
            "nested",
            "template",
            false,
            false,
            "2",
        )
        .unwrap();
        assert_eq!(out, r#"{"locals":[{"a":1},{"b":2}]}"#);
    }

    #[test]
    fn interpolations_and_functions_stay_as_template_strings() {
        let src = "locals {\n  name  = \"app-${var.env}\"\n  upper = upper(\"x\")\n  pick  = var.a ? 1 : 2\n}\n";
        let out = convert(src, "nested", "template", false, false, "2").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["locals"]["name"], "app-${var.env}");
        assert_eq!(v["locals"]["upper"], "${upper(\"x\")}");
        assert_eq!(v["locals"]["pick"], "${var.a ? 1 : 2}");
    }

    #[test]
    fn simplify_folds_constant_expressions_and_leaves_the_rest() {
        let src = "locals {\n  sum   = 1 + 2\n  same  = \"a\" == \"a\"\n  mixed = [1, 2 * 3, var.n]\n  keep  = var.region\n}\n";
        let out = convert(src, "nested", "simplify", false, false, "2").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["locals"]["sum"], 3);
        assert_eq!(v["locals"]["same"], true);
        // Element-wise folding: the constants collapse, the variable survives.
        assert_eq!(
            v["locals"]["mixed"],
            serde_json::json!([1, 6, "${var.n}"])
        );
        assert_eq!(v["locals"]["keep"], "${var.region}");
    }

    #[test]
    fn heredocs_become_plain_json_strings() {
        let src = "locals {\n  policy = <<-EOT\n    line one\n    line two\n  EOT\n}\n";
        let out = convert(src, "nested", "template", false, false, "2").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["locals"]["policy"], "line one\nline two\n");
    }

    #[test]
    fn indent_and_pretty_control_the_layout() {
        let two = convert("a = 1\n", "nested", "template", false, true, "2").unwrap();
        assert_eq!(two, "{\n  \"a\": 1\n}");
        let four = convert("a = 1\n", "nested", "template", false, true, "4").unwrap();
        assert_eq!(four, "{\n    \"a\": 1\n}");
        let tab = convert("a = 1\n", "nested", "template", false, true, "tab").unwrap();
        assert_eq!(tab, "{\n\t\"a\": 1\n}");
        let compact = convert("a = 1\n", "nested", "template", false, false, "2").unwrap();
        assert_eq!(compact, r#"{"a":1}"#);
    }

    #[test]
    fn syntax_error_reports_the_line_and_what_was_expected() {
        let err = convert("resource \"aws_instance\" \"web\" {\n  ami =\n}\n", "nested", "template", false, true, "2")
            .unwrap_err();
        assert!(err.contains("line 2"), "{err}");
        assert!(err.to_lowercase().contains("expected") || err.contains("unexpected"), "{err}");
    }

    #[test]
    fn duplicate_attribute_is_rejected() {
        let err = convert("a = 1\na = 2\n", "nested", "template", false, true, "2").unwrap_err();
        assert!(err.contains("line 2"), "{err}");
    }

    #[test]
    fn attribute_and_block_sharing_a_name_is_a_clear_error() {
        let err = convert(
            "settings = 1\n\nsettings {\n  a = 1\n}\n",
            "nested",
            "template",
            false,
            true,
            "2",
        )
        .unwrap_err();
        assert!(err.contains("conflicting definitions for 'settings'"), "{err}");
    }

    #[test]
    fn empty_input_is_rejected_with_an_example() {
        let err = convert("   \n\t\n", "nested", "template", false, true, "2").unwrap_err();
        assert!(err.contains("empty"), "{err}");
        assert!(err.contains("aws_instance"), "{err}");
    }

    #[test]
    fn comment_only_input_yields_an_empty_object() {
        let out = convert("# just a note\n", "nested", "template", false, false, "2").unwrap();
        assert_eq!(out, "{}");
    }

    #[test]
    fn unknown_option_values_are_rejected() {
        assert!(convert("a = 1", "deep", "template", false, true, "2")
            .unwrap_err()
            .contains("blocks mode"));
        assert!(convert("a = 1", "nested", "eval", false, true, "2")
            .unwrap_err()
            .contains("expressions mode"));
        assert!(convert("a = 1", "nested", "template", false, true, "8")
            .unwrap_err()
            .contains("indent"));
    }

    #[test]
    fn input_at_the_cap_is_accepted_and_one_byte_over_is_rejected() {
        let filler = "x".repeat(MAX_INPUT_BYTES - "a = \"\"\n".len());
        let at_cap = format!("a = \"{filler}\"\n");
        assert_eq!(at_cap.len(), MAX_INPUT_BYTES);
        assert!(convert(&at_cap, "nested", "template", false, false, "2").is_ok());

        let over = format!("a = \"{filler}x\"\n");
        assert_eq!(over.len(), MAX_INPUT_BYTES + 1);
        let err = convert(&over, "nested", "template", false, false, "2").unwrap_err();
        assert!(err.contains("over the 1048576 byte limit"), "{err}");
    }

    #[test]
    fn blank_option_strings_fall_back_to_the_defaults() {
        let out = convert("a = 1\n", "", "", false, true, "").unwrap();
        assert_eq!(out, "{\n  \"a\": 1\n}");
    }
}
