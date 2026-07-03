## Every Combination, One Item From Each List

The cartesian product takes one item from every list and produces all possible tuples: 2 colors × 3 sizes = 6 combinations, add 2 materials and you get 12. Paste up to four lists (items separated by newlines, commas, semicolons, or pipes — auto-detected), and the generator writes out every combination instantly. Everything runs locally in your browser; your data never leaves your device.

### Worked example

List 1 `red, blue` × List 2 `S, M, L`, joined with a space:

```
red S
red M
red L
blue S
blue M
blue L
```

The count is always the product of the list sizes (here 2 × 3 = 6), and the rightmost list cycles fastest — the same order Python's `itertools.product` or a SQL `CROSS JOIN` gives you.

### Features

- **2–4 lists**: List 1 and List 2 are required; List 3 and List 4 are optional and simply ignored when left empty.
- **Flexible splitting**: items are split by newline, comma, semicolon, or pipe (auto-detected, or pick one), trimmed, and blank entries dropped. Optionally deduplicate each list first.
- **Join your way**: space, nothing (concatenation), comma, dash, underscore, pipe, tab, or any custom string — plus an optional prefix/suffix on every line (handy for SKUs like `sku-tee-black`).
- **Three output formats**: plain lines, CSV rows (cells quoted and escaped when needed), or a JSON array of per-combination arrays.
- **Use cases**: product variant matrices, SEO keyword permutations (`best plumber Austin`), test-case grids, filename or SKU generation.

### Limits and edge cases

- The combination count is capped by **Max combinations** (default 10,000, hard cap 100,000). Exceeding it never truncates — you get an error stating the exact count so you can shrink a list or raise the cap.
- A required list with no items (empty, or only blanks/separators) is an error that names the list; empty optional lists are skipped.
- Prefix, suffix, and the join separator apply to the *lines* format only — CSV and JSON own their structure (quoting/escaping) instead.
- Items containing spaces (like `navy blue`) are fine: spaces never split items, only newlines/commas/semicolons/pipes do.

## FAQ

<details>
<summary>What order are the combinations generated in?</summary>

Odometer order: the first list varies slowest and the last list varies fastest, exactly like nested loops, Python's `itertools.product`, or a SQL `CROSS JOIN`. `red, blue` × `S, M` gives `red S`, `red M`, `blue S`, `blue M`. The input order of items within each list is preserved.

</details>

<details>
<summary>How do I combine more than four lists?</summary>

Chain runs: generate the product of the first lists, then paste the result into **List 1** of a second run (each output line becomes one item, since lines are split on newlines) and put the fifth list in **List 2**. Repeat as needed — the cap still applies to each run.

</details>

<details>
<summary>Why do I get an error instead of a truncated result for huge products?</summary>

Combination counts explode fast — four lists of 20 items are already 160,000 tuples. A silently truncated list looks complete but isn't, which is dangerous for variant matrices or test grids, so the tool refuses and tells you the exact count instead. Raise **Max combinations** (up to 100,000) or shrink a list.

</details>

<details>
<summary>Is this the same as combinations or permutations?</summary>

No. The cartesian product picks **one item from each list** (lists stay separate: color × size). Combinations and permutations pick **several items from the same list**, with order ignored or respected. If you want pairs drawn from a single list, this is not the tool for that.

</details>

<details>
<summary>Where is my list data sent?</summary>

Nowhere. Splitting, combining, and formatting run entirely in your browser via WebAssembly — there is no server call, upload, or tracking of your list contents.

</details>
