## About this tool

Paste a code snippet to identify its likely programming language without uploading the text or
calling a model. The detector uses a deterministic, GitHub-Linguist-style score table: syntax
signals, imports, keywords, shebangs, structural checks, and optional filename hints add weighted
evidence for each language. The result includes a confidence level, ranked alternatives, and the
matched signals so you can see why a language won.

Worked example: paste this Rust snippet and set the filename hint to `main.rs`:

```rust
use std::collections::HashMap;

pub fn tally(words: &[&str]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for w in words {
        *counts.entry(w.to_string()).or_insert(0) += 1;
    }
    counts
}
```

The report should identify `rust`, show the filename and Rust-specific signals in the evidence,
and list the nearest alternatives. For a short one-liner such as `print("hello")`, use the candidate
allowlist (`python,javascript,ruby`) or a filename hint to make the ambiguity explicit.

Limits and edge cases: snippets are capped at 1 MiB, the detector is heuristic rather than a trained
model, and very short or deliberately polyglot snippets can be close calls. Syntax highlighting and
file upload are intentionally separate from this tool; this page focuses on local paste-in language
detection with explainable output.

## FAQ

<details>
<summary>How accurate is the detector?</summary>

It is strongest on snippets with several lines, imports, type declarations, markup structure, or a
filename hint. It is not an ML classifier, so it does not claim corpus-level accuracy; instead it
shows the signals that matched and warns when the top candidates are close.

</details>

<details>
<summary>Why does a one-line snippet get a low-confidence warning?</summary>

Many languages share tiny snippets. `print("hello")` could be Python, Ruby-like pseudocode, or a
function call in another language. Add more context, set `filename`, enable `common_only`, or provide
a `candidates` allowlist to make a short snippet less ambiguous.

</details>

<details>
<summary>What does the candidate allowlist accept?</summary>

Use comma-separated language ids such as `rust,python,javascript` or `json,yaml,toml`. Unknown ids
produce an error that lists the supported values, and the report notes when the language pool was
restricted.

</details>

<details>
<summary>Does this upload my code or use a network service?</summary>

No. The scoring runs in local WebAssembly in the browser and the CLI/chat block uses the same pure
Rust core. The pasted snippet is not sent to a remote detector.

</details>
