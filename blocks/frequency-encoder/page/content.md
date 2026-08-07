## About this tool

Frequency Encoder replaces a categorical CSV column with how often each value
occurs — the technique usually called **frequency encoding** or **count
encoding**. A high-cardinality text column (product ID, merchant, ZIP, user
agent) becomes one numeric feature instead of the hundreds or thousands of
columns one-hot encoding would add.

Pick the column and an encoding: the raw **count**, the **frequency** share of
rows (0–1), a **percent**, or a **log-count** (`ln(1 + count)`) that compresses
very skewed distributions. Output can overwrite the column or land in a new
appended column. Rare values can be pooled into a single group so one-off
categories do not each get their own noisy level.

### Worked example

CSV:

```csv
product_id,price
X,10
Y,20
X,30
Z,40
X,50
Y,60
```

With column `product_id` and **Count** encoding, each ID becomes the number of
rows it appears in — X = 3, Y = 2, Z = 1:

```text
product_id,price
3,10
2,20
3,30
1,40
3,50
2,60
```

Switch the encoding to **Frequency** and the output to **append**, and the
original column is kept while a `product_id_freq` column holds each value's
share of the 6 rows (X = 3/6 = 0.5000, Y = 0.3333, Z = 0.1667):

```text
product_id,price,product_id_freq
X,10,0.5000
Y,20,0.3333
X,30,0.5000
Z,40,0.1667
X,50,0.5000
Y,60,0.3333
```

## Limits & edge cases

- Counts come from this one input only. There is no separate fit/transform
  split, so "unseen category" handling does not apply — every value in the data
  is counted as it is read.
- Frequency and percent divide by the number of **counted** rows. When blank
  cells are set to NaN or Zero those rows are excluded from both the counts and
  the denominator; when blanks are counted they form their own category.
- **Decimal places** apply to frequency, percent, and log-count. Raw counts are
  always whole numbers.
- Frequency encoding cannot separate two categories that occur equally often —
  they collapse onto the same number. That is inherent to the method, not a
  limit of this tool.
- Pooling below a minimum count gives every rare value the **combined** count of
  all pooled values, so they share one level. A value of 0 or 1 disables it.
- Values are trimmed before counting; with case-sensitivity off they are also
  lower-cased for grouping. The original cell text is untouched in append mode.
- One column per run. Columns are chosen by header name, or by 1-based number
  when the header checkbox is off.

## FAQ

<details>
<summary>What is frequency (count) encoding and when should I use it?</summary>

Frequency encoding replaces each category with how often it occurs in the data.
It is a good fit for high-cardinality columns where one-hot encoding would add
too many columns, and it works well when rarity itself carries signal — for
example when one-off merchants behave differently from frequent ones.

</details>

<details>
<summary>What is the difference between count, frequency, percent, and log-count?</summary>

They are the same quantity with different scaling. **Count** is the raw number
of rows. **Frequency** divides that by the number of counted rows, giving a 0–1
share. **Percent** is that share times 100. **Log-count** is `ln(1 + count)`,
which pulls in a long tail so a value seen 5,000 times does not dwarf one seen
5 times.

</details>

<details>
<summary>Two categories occur the same number of times — is that a problem?</summary>

They will receive exactly the same encoded value, because frequency encoding
only knows how often a value occurs, not which value it was. If that collision
matters for your model, keep the original column by using **append** output, or
combine this feature with another encoding.

</details>

<details>
<summary>What does pooling rare categories do?</summary>

Set a minimum count and every value occurring fewer times than that is treated
as one combined group: the pooled total replaces each of their individual
counts. This keeps a long tail of one-off values from becoming many distinct
noisy levels. Values at or above the minimum keep their own count.

</details>

<details>
<summary>How are blank cells handled?</summary>

The **Blank cells** setting decides. *Count as their own category* (the default)
treats blank as a real value and counts it like any other, which mirrors how
common encoder libraries treat missing values by default. *NaN* and *Zero* write
that marker instead and leave those rows out of the counts and the frequency
denominator.

</details>
