## About this tool

Use this browser-local CSV splitter to create reproducible train, test and optional validation sets for machine-learning experiments. Paste a CSV, choose a test size as either a fraction (`0.2`) or an absolute row count (`50`), and the tool returns labelled CSV sections, one selected split, a summary table, or JSON containing every split.

The default is an 80/20 train/test split with `seed = 42`, so the same CSV and settings always produce the same rows. Rows keep their original order inside each output split, which makes the result easier to inspect and diff.

You can stratify on a label column to keep class balance across splits, quantile-bin a numeric stratification column, group related rows so they never leak across train and test, or turn shuffling off for a sequential time-series split where the newest rows land in the test set.

## Worked example

Input:

```csv
id,label
1,a
2,b
3,a
4,b
5,a
6,b
7,a
8,b
9,a
10,b
```

With `test_size = 0.2`, `validation_size = 0`, `shuffle = false`, and `output = sections`, the rows are split sequentially:

```text
# train (8 rows)
id,label
1,a
2,b
3,a
4,b
5,a
6,b
7,a
8,b

# test (2 rows)
id,label
9,a
10,b
```

For a stratified split, keep `shuffle` on and set `stratify_column = label`. For a leakage-safe grouped split, leave `stratify_column` empty and set `group_column` to a patient, user, document, or other group identifier.

## Limits and edge cases

- `test_size` and `validation_size` below `1` are fractions of the data rows; values `1` or above are rounded to row counts.
- The split must leave at least one training row. If test plus validation consumes every row, the tool returns an error.
- `stratify_column` and `group_column` are mutually exclusive. Stratification balances classes; grouping prevents leakage, and combining both would imply a more complex grouped-stratified algorithm.
- Stratification requires `shuffle = true`. Turn shuffle off only for a sequential or time-series split.
- Grouped splits keep each group intact, so row counts can land near the requested sizes rather than exactly on them when groups are large.
- CSV input is parsed in memory. Convert Excel or Parquet to CSV first, and use a local script for very large datasets that do not fit comfortably in the browser.

## FAQ

<details>
<summary>Should I enter `0.2` or `20` for a 20% test split?</summary>

Use `0.2`. Numbers below `1` are treated as fractions of the data rows. A value of `20` means exactly twenty rows, not twenty percent.

</details>

<details>
<summary>How do I make the split reproducible?</summary>

Keep the same input CSV, the same settings, and the same `seed`. The default seed is `42`, so results are reproducible even if you never change the seed field.

</details>

<details>
<summary>When should I use stratify instead of group?</summary>

Use `stratify_column` when you want each split to preserve a label distribution, such as positive and negative classes. Use `group_column` when related rows must stay together, such as visits from the same patient or records from the same user. The tool requires you to choose one because the semantics are different.

</details>

<details>
<summary>How do I split time-series data?</summary>

Turn `shuffle` off. The tool then uses a sequential split: training rows come first, optional validation rows follow, and the test set gets the last rows.

</details>
