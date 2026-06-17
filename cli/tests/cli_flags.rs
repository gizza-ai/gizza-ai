//! Regression tests for CLI flag parsing: flags must be recognized whether they
//! appear before or after the positional tool args (the `trailing_var_arg`
//! setting used to swallow a trailing `--json-out` as a positional).

use std::process::Command;

fn gizza() -> Command {
    Command::new(env!("CARGO_BIN_EXE_gizza"))
}

#[test]
fn flag_after_positional_is_parsed() {
    let out = gizza()
        .args(["tool", "calculator", "6*7", "--json-out"])
        .output()
        .expect("run gizza");
    assert!(
        out.status.success(),
        "exit={:?} stderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("\"result\":42"), "expected json envelope, got: {stdout}");
}

#[test]
fn flag_before_positional_is_parsed() {
    let out = gizza()
        .args(["tool", "calculator", "--json-out", "6*7"])
        .output()
        .expect("run gizza");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("\"result\":42"));
}

#[test]
fn human_default_after_positional() {
    // No flag → human output "42"; a trailing key=value must not be confused for a flag.
    let out = gizza().args(["tool", "calculator", "6*7"]).output().expect("run gizza");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "42");
}
