## About this tool

Regex From Examples turns a short set of positive strings into a deterministic regular expression, then compiles the pattern and checks it against the samples before returning it. Add negative examples when you need the pattern to avoid nearby strings, choose whether to anchor the whole string, and switch between Rust, PCRE, Python, JavaScript, or POSIX-style output.

It is a structural helper, not a machine-learning synthesizer. It generalizes runs such as digits, letters, whitespace, and punctuation; when that would also match a negative example, the automatic strategy can fall back to literal alternation. Always review the pattern before using it for security-sensitive validation.

### Worked example

Positive examples:

```text
2024-01-15
2023-11-02
1999-12-31
```

Negative examples:

```text
2024/01/15
not-a-date
```

With the defaults and `output=report`, the tool emits a checked date-shaped pattern like `^\d{4}-\d{2}-\d{2}$`, explains each token run, and reports how many positives matched and negatives were excluded.

## Limits and edge cases

- The input cap is 200,000 total characters across positives and negatives.
- Each side may contain up to 5,000 examples after splitting; blank entries are ignored.
- `max_alternatives` is capped at 500 so accidental giant literal regexes fail clearly.
- The verifier uses Rust's regex engine. Flavor output changes syntax, but unsupported engine-specific features are not invented.
- A generated regex is a starting point. More examples usually produce safer, less surprising patterns.

## FAQ

<details>
<summary>Does this find the shortest possible regex?</summary>

No. It uses deterministic heuristics that prefer readable structural patterns and verified literal fallbacks. Shortest-regex synthesis can become expensive and often produces patterns that are hard for humans to maintain.

</details>

<details>
<summary>Why should I add negative examples?</summary>

Positive examples show what must match, but they do not show nearby strings that should fail. Negatives help the automatic strategy decide when a broad pattern such as `\d{3}` is too loose and when it should fall back to a tighter alternation.

</details>

<details>
<summary>What is the difference between range, open, and loose quantifiers?</summary>

`range` preserves observed lengths with `{m,n}` or `{n}`, `open` keeps only the minimum length with `{m,}`, and `loose` emits broader `+`, `*`, or `?` style quantifiers. Use `range` for validators and `loose` for exploratory search patterns.

</details>

<details>
<summary>Can I use the JavaScript output directly?</summary>

Yes for ordinary cases: JavaScript flavor is emitted as a `/pattern/flags` literal when a flag is needed. Still test it in your runtime if you depend on exact Unicode or engine-specific regex behavior.

</details>
