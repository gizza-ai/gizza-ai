use gizza_cli::args::map_args;
use serde_json::json;

fn calc_schema() -> serde_json::Value {
    json!({"type":"object","required":["expr"],"properties":{"expr":{"type":"string"}}})
}

fn resize_schema() -> serde_json::Value {
    json!({"type":"object","properties":{"url":{"type":"string"},"width":{"type":"integer"},"fit":{"type":"string"}}})
}

#[test]
fn single_required_positional() {
    assert_eq!(
        map_args(&calc_schema(), &["2*2".into()], None).unwrap(),
        json!({"expr":"2*2"})
    );
}

#[test]
fn key_value_with_type_coercion() {
    assert_eq!(
        map_args(
            &resize_schema(),
            &["url=http://x/a.png".into(), "width=640".into()],
            None
        )
        .unwrap(),
        json!({"url":"http://x/a.png","width":640})
    );
}

#[test]
fn json_escape_hatch() {
    assert_eq!(
        map_args(&resize_schema(), &[], Some(r#"{"url":"http://x","width":10}"#)).unwrap()["width"],
        10
    );
}

#[test]
fn positional_and_json_is_error() {
    assert!(map_args(&calc_schema(), &["2*2".into()], Some("{}")).is_err());
}

#[test]
fn missing_required_is_error() {
    assert!(map_args(&calc_schema(), &[], None).is_err());
}
