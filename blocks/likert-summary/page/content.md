## About this tool

Likert Scale Summary turns raw agreement/satisfaction/frequency answers into the
numbers survey reports actually quote: a **mean per item**, its **standard
deviation, median and mode**, the **full response distribution**, and the
**top-2-box / bottom-2-box** percentages — plus text **stacked bars** you can paste
straight into a doc or ticket.

Feed it either shape of data. **Responses** is the usual export: the first row
holds the item (question) headers and every later row is one respondent, with
answers as codes `1`–`N` or as the labels themselves (`Agree`, `Strongly agree`,
…). **Counts** is the tally you often get from a summary table: one row per item,
then how many respondents chose each category. Everything runs locally in your
browser — no upload, no account.

### Worked example

Seven respondents rated three items on a 5-point agreement scale:

```
Ease of use,Support,Value for money
5,2,4
4,3,4
5,1,3
4,2,5
3,4,4
5,3,4
4,2,5
```

With **Item order** set to *Highest mean first*, the item table reads:

```
Item                 n   miss     mean       sd   median    mode   Bottom 2    Neutral      Top 2
-------------------------------------------------------------------------------------------------
Ease of use          7      0     4.29     0.76     4.00     4,5       0.0%      14.3%      85.7%
Value for money      7      0     4.14     0.69     4.00       4       0.0%      14.3%      85.7%
Support              7      0     2.43     0.98     2.00       2      57.1%      28.6%      14.3%

Overall mean of item means: 3.62 (21 valid answers, 0 missing)
```

The distribution and the stacked bars follow, one row per item, with a key mapping
each character back to its category:

```
Ease of use      3333334444444444444444455555555555555555
Value for money  3333334444444444444444444444455555555555
Support          1111112222222222222222233333333333444444
Key: 1=Strongly disagree  2=Disagree  3=Neutral  4=Agree  5=Strongly agree
```

`Support` is clearly the weak item: 57.1% in the bottom two categories against
85.7% top-2-box for the other two.

### Options

- **Data shape** — *responses* (one row per respondent) or *counts* (one row per
  item holding a tally per category).
- **Item columns** — name just the Likert columns when your export also carries
  respondent IDs, timestamps, or free-text comments.
- **Scale points** — 2 to 11 categories; 4, 5 and 7 are the common ones.
- **Category labels** — agreement, satisfaction, frequency, quality, plain
  numeric, or your own comma-separated list.
- **Reverse-scored items** — flip negatively worded items with
  `new = points + 1 − answer` so every item points the same way.
- **Box size** — 2 gives the usual top-2-box / bottom-2-box; 1 gives top-box only.
- **Missing answers** — drop them item by item, or drop any respondent who skipped
  an item (listwise).
- **Stacked bars / diverging** — plain left-to-right bars, or bars centred on the
  neutral midpoint so negative and positive halves are easy to compare.
- **Cronbach's alpha** — internal-consistency reliability for the items taken as
  one scale.

### Limits and edge cases

- Answers must be whole codes from `1` to your scale-point count, or one of the
  category labels (matched case-insensitively, and by unique prefix). Anything
  else is an error naming the item and the value — so a stray ID column fails loudly
  rather than being scored as data.
- Blanks and the markers `NA`, `N/A`, `-`, `.`, `none`, `null`, `missing` and `?`
  count as missing; every other value is data.
- Means and SDs treat the scale as interval data. That is standard practice for
  reporting, but Likert answers are strictly ordinal — the median, mode and box
  percentages carry no such assumption.
- The SD is the sample SD (n − 1) and needs at least 2 answers; the median of an
  even number of answers is the average of the two middle values, so it can land on
  a half-point.
- Cronbach's alpha needs at least 2 items and at least 2 respondents who answered
  every item, and it is not available from counts input (it needs respondent-level
  rows). A negative alpha usually means an item runs the other way — reverse-score
  it and re-run.
- Percentages are always shown to 1 decimal place; the decimals setting applies to
  means, SDs, medians and alpha.
- Bars are exactly 40 characters, allocated by largest remainder, so a category
  under about 1.25% of an item can round to zero characters while still appearing
  in the distribution table.

## FAQ

<details>
<summary>What is a top-2-box score, and why use it?</summary>

Top-2-box is the share of respondents who picked either of the two most positive
categories — on a 5-point agreement scale, *Agree* plus *Strongly agree*.
Bottom-2-box is the mirror image at the negative end. They are quoted alongside the
mean because a mean of 3.5 can come from a well-liked item or from a badly split
one, while the box percentages show the split directly. Set **Box size** to 1 if you
want top-box (top category only) instead.

</details>

<details>
<summary>How do I handle reverse-worded items?</summary>

List them under **Reverse-scored items**, by header name or 1-based index. Each
answer becomes `points + 1 − answer`, so on a 5-point scale a 1 becomes a 5. Do
this before reading the item means or Cronbach's alpha: an un-reversed negative item
drags the scale mean down and can push alpha below zero.

</details>

<details>
<summary>Can I paste a summary table instead of raw responses?</summary>

Yes — switch **Data shape** to *Counts* and give one row per item: the item name,
then how many respondents chose each category, lowest category first. A header row
of category names is detected and skipped. Everything except Cronbach's alpha is
computed the same way, because means, SDs, medians and box percentages all follow
from the frequency counts.

</details>

<details>
<summary>What do the diverging bars show?</summary>

The plain bars stack every category left to right across 40 characters. The
diverging option splits each bar at the scale midpoint: negative categories extend
left of the centre line and positive ones right, with the neutral category split
across it. That lines the items up on a common centre, which makes "which items lean
negative" readable at a glance rather than requiring you to compare segment widths.

</details>

<details>
<summary>Do means make sense for Likert data?</summary>

They are contested. Likert answers are ordinal — the gap between *Disagree* and
*Neutral* is not guaranteed to equal the gap between *Agree* and *Strongly agree* —
so a mean assumes something the data does not strictly provide. In practice means
are the standard way to rank items and track them over time, which is why this tool
reports them. The median, mode, distribution and box percentages are reported next
to every mean precisely so you can check any claim against a measure that makes no
interval assumption.

</details>

<details>
<summary>What counts as a missing answer?</summary>

An empty cell, or a cell holding `NA`, `N/A`, `-`, `.`, `none`, `null`, `missing`
or `?` (any capitalization). With **Missing answers** set to *Exclude*, each item
keeps every answer it actually received and reports its own `n` and `miss` counts.
With *Listwise*, any respondent who skipped at least one item is dropped entirely,
which keeps every item based on the same people — the report says how many rows were
dropped.

</details>
