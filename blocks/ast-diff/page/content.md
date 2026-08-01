## About this tool

Rust AST Diff compares two Rust source snippets after parsing them into an abstract syntax tree and printing that tree in one canonical style. That means line wrapping, indentation, blank lines, and comments do not create noisy diff hunks. If the two snippets are the same program with different formatting, the tool says so; if a compiler-visible item changed, the unified diff is shown against the canonical Rust source.

Use it when reviewing generated Rust, checking whether a formatter changed behavior, or comparing examples where whitespace churn hides the actual code edit. Everything runs locally in your browser through WebAssembly.

Worked example — a real structural change:

Source A:

```rust
fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

Source B:

```rust
fn add(left: i32, right: i32) -> i32 {
    left + right
}
```

With **Output mode** set to `unified`, the output is a git-style diff of the canonical forms:

```diff
--- a
+++ b
@@ -1,3 +1,3 @@
-fn add(a: i32, b: i32) -> i32 {
-    a + b
+fn add(left: i32, right: i32) -> i32 {
+    left + right
 }
```

If Source B were only reformatted or had a line comment added, **summary** mode would report `formatting-only: same program, differs only in formatting/whitespace/comments`.

## Limits and edge cases

- This tool is intentionally Rust-only. Both inputs must parse as valid Rust source files or snippets accepted by `syn::parse_file`; parse errors include line and column when available.
- Formatting, whitespace, and comments are ignored because the parser discards them before canonical printing. Doc comments may become attributes in Rust's AST and can therefore appear as structural changes.
- The unified diff is produced from canonicalized lines, not original line numbers. Hunk locations refer to the canonical form.
- Large inputs are capped before building the line-diff table to avoid exhausting the browser sandbox. Diff smaller files or smaller modules if you hit the cell limit.
- `context` controls only unified diff hunks and is clamped to 0-100. `canonical-a` and `canonical-b` return one canonicalized source without comparing.

## FAQ

<details>
<summary>What does “AST diff” mean here?</summary>

The tool parses each Rust source into an abstract syntax tree, then prints that tree back in a stable style before diffing. Since formatting and line comments are not part of the tree, they disappear. Renaming a variable, changing an expression, adding an item, or modifying a type still changes the tree and appears in the diff.

</details>

<details>
<summary>Can it compare JavaScript, Python, JSON, or other languages?</summary>

No. This block uses Rust parser crates that compile cleanly to WebAssembly, so the supported language is Rust source. Other languages need their own parsers and canonical printers; mixing them into this tool would make the output less predictable and much heavier.

</details>

<details>
<summary>Why did my comments disappear?</summary>

Line comments and ordinary block comments are formatting trivia, not Rust syntax tree nodes, so they are ignored by design. That is what lets the tool suppress noisy comment and whitespace churn. Rust doc comments are different: the compiler treats them like attributes, so they may survive canonicalization and show as structural changes.

</details>

<details>
<summary>Why are the diff line numbers different from my file?</summary>

The diff is based on canonicalized Rust, not your original text. The parser and pretty-printer normalize layout before the diff is computed, so hunk headers point into that canonical form. Use `canonical-a` or `canonical-b` mode to see exactly what the diff is comparing.

</details>
