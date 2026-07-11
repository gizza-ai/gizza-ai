## About this tool

**Compare Two Lists** treats each of your two lists as a set and tells you exactly how they
overlap: which items are **only in List A**, which are **only in List B**, and which are
**shared by both** — each section with a count, followed by a totals line that also reports the
**union** size. It's the fast way to reconcile two exports of anything: email subscribers,
customer IDs, SKUs, domains, package names, or feature flags.

Everything runs locally in your browser — nothing is uploaded.

### Worked example

**List A**

```
apple
banana
cherry
```

**List B**

```
banana
cherry
date
```

**Comparison**

```
Only in A (1):
apple

Only in B (1):
date

In both (2):
banana
cherry

Totals: A=3 · B=3 · only in A=1 · only in B=1 · in both=2 · union=4
```

`apple` is unique to A, `date` is unique to B, and `banana`/`cherry` appear in both.

### Options

- **Separator** — split each list by new line (default), comma, tab, semicolon, pipe, or space.
- **Ignore case** — treat `Apple` and `apple` as the same item.
- **Trim whitespace** — strip leading/trailing spaces before comparing (on by default).
- **Ignore blank items** — drop empty lines (on by default).
- **Remove duplicates** — collapse repeats within a list so it behaves as a true set (on by
  default; turn it off to keep every occurrence).
- **Ignore leading zeros** — match `007` with `7`, handy for numeric IDs.
- **Sort order** — keep the input order (default), or sort each section A→Z or Z→A.

### Limits

- Up to 100,000 items per list.
- The item text you paste is compared as-is (after the options above) — there is no fuzzy or
  partial matching; `apple` and `apples` are different items.
- The displayed item keeps List A's original form when the same item is shared (so with
  **Ignore case** on, a shared `Bob@x.com` shows A's capitalization).

## FAQ

<details>
<summary>What is the difference between "only in A", "only in B", and "in both"?</summary>

**Only in A** are items present in your first list but missing from the second. **Only in B** are
items in the second list but missing from the first. **In both** are the items the two lists
share (the intersection). Together they make up the **union**, whose size is shown on the totals
line.

</details>

<details>
<summary>How do I compare comma-separated or tab-separated lists?</summary>

Change the **Separator** dropdown to `Comma`, `Tab`, `Semicolon`, `Pipe`, or `Space`. Both lists
are split the same way, so paste them in whatever format you have — one item per line is the
default.

</details>

<details>
<summary>Does it ignore duplicates and blank lines?</summary>

By default yes. **Remove duplicates** collapses repeated items within each list so the comparison
uses set semantics, and **Ignore blank items** drops empty lines. Turn **Remove duplicates** off
if you want every occurrence listed instead. Note the totals counts are always unique-set based,
so `union` always equals `only in A + only in B + in both`.

</details>

<details>
<summary>Can it match items that differ only by case or leading zeros?</summary>

Yes. Enable **Ignore case** to treat `Apple` and `apple` as the same item, and **Ignore leading
zeros** to treat `007` and `7` as equal — useful for reconciling numeric IDs, order numbers, or
SKUs that were exported with different zero-padding.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The comparison runs entirely in your browser via WebAssembly — your lists never leave your
device.

</details>
