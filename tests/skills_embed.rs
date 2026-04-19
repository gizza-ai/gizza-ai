#[test]
fn clock_skill_is_embedded() {
    assert!(
        gizza_ai::skills::SKILLS.iter().any(|(n, _)| *n == "gizza-ai/clock"),
        "gizza-ai/clock should be embedded — did you forget to build the block first?"
    );
    let (_, bytes) = gizza_ai::skills::SKILLS
        .iter()
        .find(|(n, _)| *n == "gizza-ai/clock")
        .expect("clock");
    assert!(!bytes.is_empty(), "clock wasm bytes non-empty");
    // Quick wasm magic-number check.
    assert_eq!(&bytes[..4], b"\0asm", "clock bytes look like wasm");
}
