#[test]
fn single_example_terminates() {
    let out = gizza_ai_regex_from_examples_core::render(
        "ab", "", "", "", "", "", true, false, false, "pattern", 20.0,
    )
    .unwrap();
    assert_eq!(out, r"^[a-z]{2}$");
}
