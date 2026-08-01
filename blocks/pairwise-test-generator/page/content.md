## Turn a parameter model into a compact test matrix

Testing every combination of every option quickly explodes: three parameters with 3, 3, and 2 values already make 18 full combinations, and adding a fourth parameter multiplies that again. **Pairwise (all-pairs) testing** exploits the fact that most defects are triggered by a single value or by the interaction of just *two* values. This tool generates a small set of test cases in which **every value of every parameter is paired at least once with every value of every other parameter** — so you keep the coverage that finds interaction bugs while dropping most of the redundant rows.

List one parameter per line as `Name: value1, value2, …`. Blank lines and lines that start with `#` are ignored, so you can annotate your model. The generator is fully deterministic — the same model always produces the same cases — and runs entirely in your browser, so nothing you type is uploaded.

Choose **Markdown** for a table you can paste into a pull request or wiki, **CSV** to import into a spreadsheet or test-management tool, **JSON** to feed a script or data-driven test runner, or **ASCII** for a plain box-drawn grid. Leave **Number each case** on to prepend a `#` column, or turn it off for a clean copy.

### Worked example

Model:

```text
Browser: Chrome, Firefox, Safari
OS: Windows, macOS, Linux
Theme: Light, Dark
```

The full Cartesian product is 3 × 3 × 2 = **18** combinations. Pairwise coverage needs only **9**:

```markdown
| # | Browser | OS      | Theme |
| --- | ------- | ------- | ----- |
| 1 | Chrome  | Windows | Light |
| 2 | Chrome  | macOS   | Dark  |
| 3 | Chrome  | Linux   | Light |
| 4 | Firefox | Windows | Dark  |
| 5 | Firefox | macOS   | Light |
| 6 | Firefox | Linux   | Dark  |
| 7 | Safari  | Windows | Light |
| 8 | Safari  | macOS   | Dark  |
| 9 | Safari  | Linux   | Light |
```

Every Browser/OS, Browser/Theme, and OS/Theme pair appears in at least one row — for example `Firefox` is tested with `Windows`, `macOS`, and `Linux`, and with both `Light` and `Dark`.

### FAQ

<details>
<summary>What is pairwise (all-pairs) testing?</summary>

It is a test-design technique that generates the smallest practical set of cases so that **every pair of values from any two parameters** is exercised at least once. Empirically most bugs depend on one factor or the interaction of two, so all-pairs coverage catches the large majority of interaction defects for a tiny fraction of the exhaustive cost.

</details>

<details>
<summary>How do I write the parameter model?</summary>

One parameter per line as `Name: value1, value2, …`. The part before the first colon is the parameter name (it may itself contain commas or spaces); everything after is a comma-separated list of values. Blank lines and lines beginning with `#` are ignored, so `# smoke test` is a comment. You need **at least two** parameters, because a pair needs two.

</details>

<details>
<summary>Will I get the same cases every time?</summary>

Yes. The generator uses a deterministic greedy algorithm — it seeds each new case from the first still-uncovered pair, then fills the remaining parameters with the value that covers the most still-missing pairs, breaking ties by order. The same model always yields byte-for-byte identical output, so you can commit the result and diff it.

</details>

<details>
<summary>Why did I not get the absolute minimum number of cases?</summary>

Finding the provably smallest all-pairs set is NP-hard. This tool uses a fast greedy heuristic that produces a near-minimal set — usually within a case or two of optimal for typical models — while guaranteeing full pair coverage. It favours speed and determinism over squeezing out the last row.

</details>

<details>
<summary>Can I add constraints like "if OS is Linux, Browser cannot be Safari"?</summary>

Not yet — this tool generates unconstrained **pairwise** (2-way) coverage only. It does not support constraints/exclusions, seeded mandatory rows, or higher-strength *t*-way (3-way, 4-way) coverage. After generating, delete any rows that violate a real constraint, or split your model so invalid pairs never occur.

</details>

### Limits and edge cases

- **Pairwise only.** Coverage is strictly 2-way (all-pairs). There is no 3-way/4-way (*t*-way) mode and no way to guarantee a specific triple of values appears together.
- **No constraints or exclusions.** Every combination is treated as valid; the generator will happily pair values that your system forbids. Prune impossible rows afterward.
- **At least two parameters.** A single parameter has no pairs to cover, so the tool asks for two or more.
- **Size caps.** Up to **20 parameters** and **30 values** per parameter. Values within one parameter must be unique, and parameter names must not repeat — duplicates are rejected with a clear message.
- **Near-minimal, not provably minimal.** The greedy result is compact but may include a case or two more than a theoretical optimum.
