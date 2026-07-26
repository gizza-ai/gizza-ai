## About this tool

Code Chunker splits a source file into chunks that line up with top-level functions,
classes, methods, imports, or statements. It is useful when you need stable chunks for
embeddings, retrieval-augmented generation, code review prompts, or smaller LLM context
windows without cutting through the middle of a definition.

Choose a language, set a target maximum number of lines, and pick JSON, JSON Lines, or
plain text output. Each JSON record includes the chunk index, 1-based start and end
lines, line count, best-effort construct kind/name, an `oversize` flag, and the chunk
text. Consecutive small top-level units are packed together up to `max_lines`; a single
definition that is larger than the target stays whole and is marked `oversize`.

### Worked example

Input Rust:

```rust
fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn sub(a: i32, b: i32) -> i32 {
    a - b
}
```

With `language = rust`, `max_lines = 3`, and `format = json`, the output is two records:
one for `add` on lines 1-3 and one for `sub` on lines 5-7. Leading comments and Python
decorators are attached to the construct they describe, so documentation usually travels
with the code it documents.

### Limits and edge cases

- This is a deterministic heuristic, not a full syntax-tree parser. Brace languages are
  segmented by bracket balancing while ignoring brackets in strings and comments; Python
  is segmented by indentation.
- Unusual macro syntax, here-docs, code generated inside strings, or malformed code can
  produce best-effort boundaries.
- The size budget is measured in lines, not tokens or characters. Use a token counter
  after chunking if you need exact model-token budgets.
- Oversize definitions are not recursively split because this tool prioritizes keeping a
  definition intact.

## FAQ

<details>
<summary>Does Code Chunker upload my source code?</summary>

No. The page runs the Rust/Wasm chunker locally in your browser. The CLI and chat block
also run the same deterministic core logic without a network service.

</details>

<details>
<summary>Is this a real parser for every supported language?</summary>

No. It is parser-like enough for common top-level function and class boundaries, but it
does not load per-language grammar engines. That keeps the tool small and compatible
with the pure Wasm sandbox, while still returning line ranges and construct names for
common Python and brace-language files.

</details>

<details>
<summary>What happens when a function is longer than max_lines?</summary>

The function or class stays in one chunk and the JSON record sets `oversize` to `true`.
That avoids splitting in the middle of a definition; downstream code can decide whether
to summarize, reject, or manually split that oversize chunk.

</details>

<details>
<summary>When should I choose JSON Lines instead of JSON?</summary>

Use JSON Lines when you want to stream or pipe chunks one record per line into another
tool. Use pretty JSON for inspection and plain text when you want paste-ready chunks with
visible header banners.

</details>
