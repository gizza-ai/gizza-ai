## About this tool

Class Rebalancer evens out an imbalanced label column in a CSV by resampling
**whole rows** — randomly duplicating rows from the smaller class(es) to bring
them up (over-sampling) and/or randomly dropping rows from the larger class(es)
to bring them down (under-sampling), toward a target class ratio you choose.

Pick the label column and a strategy, and every class moves toward the same size
(or a ratio you set). The picks are driven by a fixed **seed**, so the same input
and seed always produce the same output — change the seed for a different draw.
Everything runs locally in your browser; no rows are uploaded, and no synthetic
rows are invented (this is random resampling, not SMOTE).

### Worked example

CSV — one `spam` row and three `ham` rows (a 1:3 imbalance):

```csv
text,label
buy now,spam
hello,ham
hi there,ham
see you,ham
```

With label column `label`, strategy **oversample**, and target ratio `1.0`, the
minority `spam` class is duplicated up to the majority size (3), keeping the
originals first and appending the duplicates:

```text
text,label
buy now,spam
hello,ham
hi there,ham
see you,ham
buy now,spam
buy now,spam
```

Switch **Output** to the before/after summary to get a JSON count report instead
of the CSV:

```json
{
  "classes": [
    {"label": "spam", "before": 1, "after": 3},
    {"label": "ham", "before": 3, "after": 3}
  ],
  "strategy": "oversample",
  "target_ratio": 1,
  "total_before": 4,
  "total_after": 6
}
```

## Limits & edge cases

- **Oversample** only grows classes (duplicates minority rows); **undersample**
  only shrinks classes (drops majority rows); **combine** moves every class to a
  common size, growing the small ones and shrinking the big ones. **Auto** is the
  same as oversample.
- **Target ratio** is the desired minority-to-majority ratio after resampling,
  from just above 0 to 1.0. `1.0` = fully balanced (every class equal); `0.5` =
  the smaller class ends at half the larger. Values of 0 or above 1 are rejected.
- The label column can be a header name (when the header box is on) or a 1-based
  column number. Blank means the last column.
- You need at least two distinct classes in the label column — a single-class
  column has nothing to rebalance.
- Over-sampling duplicates existing rows verbatim, so it cannot add new
  information; it just reweights the classes. A very small target ratio combined
  with over-sampling can request an unrealistically large table, which is capped.
- With the same seed, the output is identical every time. Turn on **shuffle** to
  interleave the duplicates instead of appending them at the end.

## FAQ

<details>
<summary>What is class imbalance and why rebalance?</summary>

Class imbalance is when one label dominates your dataset — say 95% "not fraud"
and 5% "fraud". Many models learn to just predict the majority and still look
accurate, while missing the class you care about. Rebalancing evens the class
counts so the training signal is more even; over-sampling repeats minority rows,
under-sampling drops majority rows.

</details>

<details>
<summary>Does this create synthetic rows like SMOTE?</summary>

No. This tool only **copies or removes whole existing rows** — every output row
is an exact row from your input. It does not interpolate or generate new
synthetic samples between points the way SMOTE does, so it never invents feature
values that weren't in your data.

</details>

<details>
<summary>Why is there a seed, and is the output reproducible?</summary>

The seed drives the reproducible pseudo-random picks for which rows to duplicate
or drop (and the optional shuffle). The same input and seed always give the same
output, so your rebalanced set is fully reproducible; change the seed to get a
different random draw.

</details>

<details>
<summary>What's the difference between oversample, undersample, and combine?</summary>

**Oversample** duplicates minority-class rows until they reach the target ratio,
keeping every majority row. **Undersample** drops majority-class rows down to the
target, keeping every minority row. **Combine** does both, moving every class to a
common size. Oversample keeps all your data but repeats some; undersample throws
data away but adds no duplicates.

</details>
