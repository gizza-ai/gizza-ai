## About this tool

Use this combinatorics calculator for lottery odds, poker hands, seating charts, password/PIN spaces, sampling plans, or any case where you need to choose or arrange `r` items from a pool of `n`. The summary view prints the exact count, the formula with your values substituted, whether order matters, whether repetition is allowed, and the odds of one specific result.

Paste an item list when you need the actual generated combinations or permutations. The tool can split comma-separated lists, spreadsheet columns, newlines, semicolons, pipes, tabs, or spaces; it can also deduplicate repeated pasted values. Enumeration can be rendered as one line per result, CSV rows, or JSON arrays.

### Worked example

For a 6/49 lottery draw, set `n=49`, `r=6`, `mode=combinations`, and leave repetition off. The count is:

```text
C(49, 6) = 13,983,816
```

That means one specific ticket has odds of 1 in 13,983,816. If you paste `A, B, C, D`, set `r=2`, and choose `output_format=lines`, the generated combinations are:

```text
A, B
A, C
A, D
B, C
B, D
C, D
```

### Limits and edge cases

Counting accepts `n` and `r` up to 1000 and uses exact arbitrary-precision arithmetic, so large values such as `C(1000, 500)` are printed without floating-point rounding. Enumeration is capped by `max_results` (default 10,000, hard cap 100,000); if the requested list is larger, the tool reports the exact total and asks you to narrow the request instead of silently truncating. Circular permutations require `r >= 1` and do not allow repetition because repeated circular arrangements are necklace-counting, a different model.

## FAQ

<details>
<summary>When should I choose combinations instead of permutations?</summary>

Choose combinations when order does not matter. A lottery ticket `{1, 2, 3, 4, 5, 6}` is the same ticket no matter how the numbers are ordered, so it uses `C(n, r)`. Choose permutations when order matters, such as ranking first/second/third place or generating PINs.

</details>

<details>
<summary>What does "allow repetition" mean?</summary>

Repetition means the same item can be selected more than once. For permutations this is `n^r`, such as 10 digits for each of 3 PIN slots. For combinations it is `C(n + r - 1, r)`, such as choosing scoops of ice cream when the same flavor can appear multiple times.

</details>

<details>
<summary>Can I generate the actual list instead of just counting?</summary>

Yes. Paste items into the Items field, set `r`, then choose `output_format=lines`, `csv`, or `json`. If Items is empty, enumeration uses the numeric pool `1..n`, which is useful for dice, lotteries, or quick checks.

</details>

<details>
<summary>Why does a large enumeration return an error?</summary>

The browser can count huge spaces instantly, but listing every result can produce millions of rows. `max_results` protects the tab and the CLI. The error includes the exact count so you can decide whether to lower `r`, shrink the pool, enable dedupe, or switch to `output_format=count`.

</details>
