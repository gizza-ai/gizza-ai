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
fn positional_value_containing_equals_is_not_key_value() {
    // A bare positional whose VALUE contains `=` (no matching schema key
    // before it) must map to the required field, not be misread as key=value.
    assert_eq!(
        map_args(&calc_schema(), &["a=b".into()], None).unwrap(),
        json!({"expr":"a=b"})
    );
    // files-to-prompt's real input starts with the `===` separator.
    let ftp = json!({"type":"object","required":["files"],
        "properties":{"files":{"type":"string"},"format":{"type":"string"}}});
    assert_eq!(
        map_args(&ftp, &["=== a.txt\nhi".into()], None).unwrap(),
        json!({"files":"=== a.txt\nhi"})
    );
    // A real key=value still wins when the key names a property.
    assert_eq!(
        map_args(&ftp, &["=== a.txt\nhi".into(), "format=plain".into()], None).unwrap(),
        json!({"files":"=== a.txt\nhi","format":"plain"})
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
