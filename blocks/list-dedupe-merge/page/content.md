## About this tool

**Merge Two Lists & Remove Duplicates** combines your two lists into a single, clean list with
every duplicate collapsed — the set **union** of the two — and tells you exactly how much overlap
it removed. It's the fast way to consolidate two exports of anything: email subscribers, customer
IDs, SKUs, domains, tags, package names, or feature flags into one deduplicated list you can copy
straight out.

The totals line reports each list's size, the merged size, how many **duplicates** were removed,
and how many entries were **shared by both** lists (the cross-list overlap) — the headline number
when you want to know how much two lists have in common.

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

**Merged list**

```
Merged (4):
apple
banana
cherry
date

Totals: A=3 · B=3 · merged=4 · duplicates removed=2 · shared by both=2
```

The two lists had 6 entries between them; `banana` and `cherry` appeared in both, so they were
collapsed to one each. The result is 4 unique items, 2 duplicates removed, and 2 entries shared
by both lists.

### Options

- **Separator** — split each list by new line (default), comma, tab, semicolon, pipe, or space.
- **Merge order** — **Append** puts all of List A first, then List B (default); **Interleave**
  alternates A, B, A, B… When the same item appears twice, the first occurrence is the one kept,
  so merge order decides which list's copy (and its exact spelling/case) survives.
- **Trim whitespace** — strip leading/trailing spaces before comparing (on by default).
- **Ignore blank items** — drop empty lines (on by default).
- **Ignore case** — treat `Apple` and `apple` as the same item (the kept copy shows the first
  occurrence's original case).
- **Sort order** — keep the input order (default), or sort the merged list A→Z or Z→A.
- **Ignore leading zeros** — match `007` with `7`, handy for numeric IDs.

### Limits

- Up to 100,000 items per list.
- Deduplication is always on — that is the whole point. To *see* which items are only in A, only
  in B, or shared, use a set-difference tool instead; this tool produces the combined list.
- Items are compared as-is (after the options above) — there is no fuzzy or partial matching, so
  `apple` and `apples` stay distinct.
- The merged list is joined one item per line for readability, whatever separator you split on.

## FAQ

<details>
<summary>How is this different from just comparing two lists?</summary>

A comparison (set-difference) tool shows you three buckets — only in A, only in B, and shared.
This tool does the opposite job: it gives you the single combined list with duplicates removed
(the **union**), which is what you want when you're consolidating two exports into one. The
totals line still tells you how many entries the two lists shared.

</details>

<details>
<summary>What does "duplicates removed" versus "shared by both" mean?</summary>

**Duplicates removed** is every entry that was collapsed, including repeats *within* a single
list and matches *across* the two lists — it always equals `A + B − merged`. **Shared by both**
counts only the distinct entries that appeared in *both* List A and List B (the cross-list
overlap). If neither list has internal repeats, the two numbers match.

</details>

<details>
<summary>How do I merge comma-separated or tab-separated lists?</summary>

Change the **Separator** dropdown to `Comma`, `Tab`, `Semicolon`, `Pipe`, or `Space`. Both lists
are split the same way, so paste them in whatever format you have — one item per line is the
default. The merged output is always joined one item per line.

</details>

<details>
<summary>Which copy is kept when an item appears in both lists?</summary>

The first occurrence wins, so **Merge order** decides it: with **Append**, List A's copy is kept;
with **Interleave**, whichever list reached that item first. This matters with **Ignore case**
on — a shared `Bob@x.com` keeps the capitalization of the copy that appeared first.

</details>

<details>
<summary>Can it match items that differ only by case or leading zeros?</summary>

Yes. Enable **Ignore case** to treat `Apple` and `apple` as the same item, and **Ignore leading
zeros** to treat `007` and `7` as equal — useful for consolidating numeric IDs, order numbers, or
SKUs that were exported with different zero-padding.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The merge runs entirely in your browser via WebAssembly — your lists never leave your device.

</details>
