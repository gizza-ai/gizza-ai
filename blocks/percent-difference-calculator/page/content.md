## About the percent difference calculator

Enter two numbers and this calculator reports how far apart they are, three ways
at once:

- **Absolute difference** — `|a − b|`, plus the signed difference `b − a` and
  the direction of the move (increase, decrease, or no change).
- **Percent difference** (symmetric) — `|a − b| ÷ |(a + b) / 2| × 100`. The gap
  is measured against the **midpoint** of the two values, so neither value is
  privileged as "the" baseline and swapping `a` and `b` gives the same answer.
  Use this when the two numbers are just two measurements, with no before/after
  relationship — two lab readings, two quotes, two sensors.
- **Percent change** (directional) — `(b − a) ÷ |a| × 100`, the change from `a`
  to `b` measured against the **starting** value, reported in both directions
  along with the ratio `b ÷ a`. Use this when `a` really is the baseline: last
  month's revenue, the old price, the control group.

Set **Measures to report** to `difference` or `change` to show only one block,
and **Decimal places** (0–10) to control display precision. The arithmetic
itself is always done at full double precision; the setting only rounds what is
printed. Everything runs locally in your browser — nothing is uploaded, and it
keeps working offline once the page has loaded.

### Worked example: 70 and 85

| Step | Calculation | Result |
| --- | --- | --- |
| Absolute difference | `abs(70 − 85)` | `15` |
| Mean (midpoint) | `(70 + 85) / 2` | `77.5` |
| Percent difference | `15 ÷ 77.5 × 100` | **`19.3548%`** |
| Percent change a → b | `15 ÷ 70 × 100` | **`21.4286%`** increase |
| Percent change b → a | `−15 ÷ 85 × 100` | `−17.6471%` |
| Ratio b / a | `85 ÷ 70` | `1.2143` |

The two percentages differ because they divide by different things: `77.5` (the
midpoint) versus `70` (the starting value). Both are correct answers to
different questions, which is why they are shown side by side.

### More examples

- `5` and `7` → percent difference `33.33%` (mean `6`, difference `2`). Swap
  them and it is still `33.33%`.
- `25` and `75` → percent difference exactly `100%` — the symmetric measure hits
  100% precisely when one value is three times the other, not when it doubles.
- `120` → `100` → percent change `−16.67%`, ratio `0.83`. Reverse the direction
  and the same pair is a `+20%` change, because the baseline changed.
- `−70` and `−85` → percent difference `19.3548%`, the same as the positive
  pair: magnitudes are compared, so a sign flip on both values changes nothing.
- `10` and `6` at 0 decimal places → percent difference `50%`.

### Limits and edge cases

- **Percent difference is undefined when `a + b = 0`** (for example `5` and
  `−5`). The mean is zero, so there is nothing to measure the gap against. In
  `all` mode the measure is omitted with a note; in `difference` mode it is an
  error, because that is the measure you asked for.
- **Percent change is undefined when `a = 0`.** There is no baseline to divide
  by — any move away from zero is an infinite percent change. Same handling: a
  note in `all` mode, an error in `change` mode. When `b = 0`, only the reverse
  direction `b → a` and the ratio drop out.
- **Values more than 10× apart get an advisory note.** The symmetric percent
  difference saturates towards 200% for very unequal values, so it stops
  discriminating; percent change is the more informative measure there.
- **200% is the ceiling** of the symmetric measure for two same-signed values,
  reached in the limit as one of them approaches zero.
- **Negative values are accepted everywhere.** Percent change divides by `|a|`
  so that the sign of the result always matches the sign of `b − a`, rather than
  flipping on a negative baseline.
- **`NaN` and infinity are rejected** with a message naming the field.
- **Decimal places accept 0 to 10.** Anything higher is an error rather than a
  silently clamped value.
- The measure is unit-free: both values must be in the same unit, and the result
  is a percentage either way. Percentages themselves are numbers, so comparing
  `20` and `30` percent gives a percent difference of `40%` — a difference of 10
  **percentage points**.

### FAQ

<details>
<summary>What is the difference between percent difference and percent change?</summary>

Percent change has a direction: it divides by the **starting** value, so
going from 100 to 120 is a 20% increase while going from 120 to 100 is a
16.67% decrease — the same pair of numbers, two different answers. Percent
difference divides by the **mean** of the two values instead, so it is
symmetric: 100 and 120 are 18.18% apart no matter which you name first. Pick
change when one value is a baseline, difference when the two values are peers.

</details>

<details>
<summary>Why does swapping the two values not change the percent difference?</summary>

Because the formula uses `|a − b|` and the mean `(a + b) / 2`, and both are
unchanged when you swap the inputs. That is the whole point of the symmetric
measure — it does not force you to pick which value is the reference. The
trade-off is that it cannot be inverted: knowing the percentage and one value
is not enough to recover the other, since the absolute value has thrown away
the sign.

</details>

<details>
<summary>Can the percent difference be more than 100%?</summary>

Yes. It reaches exactly 100% when one value is three times the other (for
example 25 and 75), and climbs towards a ceiling of 200% as one of two
same-signed values approaches zero. Because of that ceiling, values that are
orders of magnitude apart all report a percentage close to 200% — the
calculator adds a note past a 10× ratio suggesting percent change instead.

</details>

<details>
<summary>What happens with zero or with negative numbers?</summary>

Negative numbers are fine. Percent difference compares magnitudes, so `−70`
and `−85` are 19.3548% apart just like `70` and `85`. Zeros are where measures
genuinely stop existing: percent change needs a non-zero starting value, and
percent difference needs a non-zero mean, so `5` and `−5` has no percent
difference at all. In `all` mode the undefined measure is dropped and
explained; in a single-measure mode it is reported as an error.

</details>

<details>
<summary>Is percent difference the same as percent error?</summary>

No. Percent error compares a measurement against a known true value and
divides by that true value, which makes it directional like percent change.
Percent difference is for two values of equal standing where neither is the
accepted reference. If one of your numbers is a reference or expected value,
use the change measure with that value as `a`.

</details>

<details>
<summary>Is it free and private?</summary>

Yes. The calculation runs entirely in your browser as WebAssembly — the
numbers you type never leave your device, no account is needed, and the page
keeps working with no network connection.

</details>
