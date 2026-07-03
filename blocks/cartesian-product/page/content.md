## Every Combination, One Item From Each List

The cartesian product takes one item from every list and produces all possible tuples: 2 colors × 3 sizes = 6 combinations, add 2 materials and you get 12. Paste up to four lists (items separated by tabs, newlines, commas, semicolons, or pipes — auto-detected, so spreadsheet cells paste straight in), and the generator writes out every combination instantly. Everything runs locally in your browser; your data never leaves your device.

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
- **Flexible splitting**: items are split by tab, newline, comma, semicolon, or pipe (auto-detected, or pick one — slash is available but never guessed, so URLs and dates stay whole), trimmed, and blank entries dropped. Optionally deduplicate each list first. Tab mode also splits on line breaks, so a rectangle of spreadsheet cells pastes in as one item per cell.
- **Join your way**: space, nothing (concatenation), comma, dash, underscore, pipe, tab, newline, or any custom string — plus an optional prefix/suffix on every line. That covers SKUs (`sku-tee-black`), quoted PPC phrase-match keywords (prefix and suffix `"`), bracketed `[...]` variants, and `+broad +match` terms (custom join `" +"` with prefix `+`).
- **Count before you generate**: the *Count only* output format reports the multiplication (`2 x 3 x 2 = 12`) without producing a single line — and it ignores the cap, so you can size up a product that would be too big to generate.
- **Four output formats**: plain lines, CSV rows (cells quoted and escaped when needed), a JSON array of per-combination arrays, or the count alone. Copy the result or download it as a text file.
- **Use cases**: product variant matrices, SEO keyword permutations (`best plumber Austin`), test-case grids, filename or SKU generation.

### Limits and edge cases

- The combination count is capped by **Max combinations** (default 10,000, hard cap 100,000). Exceeding it never truncates — you get an error stating the exact count so you can shrink a list or raise the cap. The *Count only* format is exempt: counting `26 × 26 × 26 × 26 = 456,976` is instant and safe even though generating it would exceed the cap.
- A required list with no items (empty, or only blanks/separators) is an error that names the list; empty optional lists are skipped.
- Prefix, suffix, and the join separator apply to the *lines* format only — CSV, JSON, and Count own their structure (quoting/escaping) instead.
- Items containing spaces (like `navy blue`) are fine: spaces never split items, only tabs/newlines/commas/semicolons/pipes do. Numeric-looking items like `007` or `1.50` stay exactly as typed — nothing is reformatted as a number.

## FAQ

<details>
<summary>What order are the combinations generated in?</summary>

Odometer order: the first list varies slowest and the last list varies fastest, exactly like nested loops, Python's `itertools.product`, or a SQL `CROSS JOIN`. `red, blue` × `S, M` gives `red S`, `red M`, `blue S`, `blue M`. The input order of items within each list is preserved.

</details>

<details>
<summary>Can I paste straight from a spreadsheet?</summary>

Yes. A pasted column arrives newline-separated and a pasted row arrives tab-separated — auto-detection handles both, and tab mode also splits on line breaks, so even a rectangular block of cells becomes one item per cell. Numbers keep their exact formatting (`007` stays `007`).

</details>

<details>
<summary>How do I know how many combinations I'll get before generating?</summary>

Set **Output format** to *Count only*. It reports the multiplication — for example `2 x 3 x 2 = 12` — without generating anything, and it is exempt from the Max combinations cap, so you can safely size up products in the millions before deciding how to split the work.

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
