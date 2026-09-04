## About this tool

Generate example strings that match a regular expression without sending the pattern anywhere. It is useful when you need stable test fixtures, sample rows for documentation, seed values for fuzzing, or a quick way to prove that a validation regex accepts the shape you expect.

The generator supports the regex constructs that map cleanly to finite strings in a small browser tool: literals, escaped characters, `.`, character classes and ranges, `\d`, `\w`, `\s` and their negations, groups, alternation, anchors, and the quantifiers `?`, `*`, `+`, `{n}`, `{n,}` and `{n,m}`. Unsupported constructs fail with a direct message instead of producing misleading samples.

### Worked example

For an order-code pattern:

```
[A-Z]{3}-\d{4}
```

Use `count=5`, `style=random`, `seed=42`, `max_repeat=4`, `max_length=200`, `unique=true`, and `output=lines`. The same settings always produce the same five samples, so you can paste the output into a test fixture and regenerate it later without churn.

Switch `style` to `sequential` when you want systematic coverage of alternations and character classes, or choose `shortest` / `longest` to see the minimum or maximum string this tool will emit under the current repeat cap.

### Limits and edge cases

- Patterns are capped at **2000 characters**.
- `count` accepts **1 to 200** samples per run.
- `max_repeat` accepts **1 to 50** and caps unbounded repeats such as `*`, `+`, and `{n,}`. Explicit upper bounds larger than the cap are reduced for generation.
- `max_length` accepts **1 to 2000** characters per generated sample. If the shortest possible match is already longer than that, the run fails and tells you to raise the limit or simplify the pattern.
- Anchors such as `^`, `$`, `\A`, `\z`, and `\Z` are treated as zero-width and do not appear in the output.
- Lazy suffixes are accepted as the same language as their greedy form, because generation cares about strings that match, not engine backtracking preference.
- Lookaround, backreferences, inline flags, atomic or possessive quantifiers, word-boundary assertions, POSIX classes, and Unicode property escapes are out of scope and are rejected clearly.
- `unique=true` may return fewer rows than `count` when the pattern has a small finite language, such as `a?`.

## FAQ

<details>
<summary>Is this a full replacement for a regex engine?</summary>

No. It generates strings for a practical, finite subset of regex syntax. Features that depend on engine state or previous captures — for example backreferences, lookaround and word boundaries — are deliberately rejected because a small deterministic generator would otherwise have to guess engine-specific behaviour.

</details>

<details>
<summary>Why do `*`, `+` and `{n,}` stop after a few repeats?</summary>

Those quantifiers describe infinitely many possible strings, so a browser tool needs a cap. `max_repeat` sets that cap. A pattern like `ab+` with `max_repeat=4` can emit `ab`, `abb`, `abbb` and `abbbb`; raising the cap explores longer matches while `max_length` still protects the page from runaway output.

</details>

<details>
<summary>How do I get reproducible random-looking samples?</summary>

Use `style=random` and keep the same `seed`. The generator uses deterministic pseudo-random choices, so the same pattern and settings produce the same output every time. Change the seed when you want a different fixture set without changing the regex itself.

</details>

<details>
<summary>When should I use sequential, shortest or longest style?</summary>

Use `sequential` for coverage: it walks choices in odometer order and is good for alternations like `(red|green|blue)`. Use `shortest` to check minimum accepted input and `longest` to inspect the largest sample this tool will emit under `max_repeat` and `max_length`.

</details>

<details>
<summary>Does generated output prove my regex is correct?</summary>

It proves that the samples are in the language this tool understands, not that the regex captures every real-world case. Use the samples as fixtures and sanity checks, then still test the regex in the engine and flavour your application actually uses.

</details>
