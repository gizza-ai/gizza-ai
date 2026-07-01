## About this tool

The **moving average calculator** smooths a number series by averaging values
over a sliding look-back window. It computes the three most widely used moving
averages side by side:

- **Simple moving average (SMA)** — the unweighted mean of the last *period*
  values. Every point in the window counts equally.
- **Exponential moving average (EMA)** — a weighted average that reacts faster to
  recent values. It uses the smoothing factor `k = 2 / (period + 1)` and is seeded
  with the simple mean of the first window, the standard finance convention.
- **Weighted moving average (WMA)** — uses linear weights `1, 2, …, period` so the
  most recent value carries the most weight and the oldest the least.

Paste your series (separated by spaces, commas, semicolons, or newlines), choose a
**period** (the window size), and the tool returns the SMA and EMA at every point.
During the *warm-up* region — before enough points are available to fill the
window — the value is reported as `null`.

## How to use it

1. Paste or type your numbers into the **Number series** box.
2. Set the **Period** — e.g. `3` for a 3-point average, `20` for a 20-day average.
3. Read off the SMA and EMA arrays, one entry per input point.

## Common uses

- Smoothing **stock or crypto price** data for technical analysis.
- Filtering noise out of **sensor readings** or **metrics**.
- Spotting **trends** in sales, traffic, or any sequential data.

## Privacy

Everything runs locally in your browser via WebAssembly. Your data is never
uploaded to a server.

## FAQ

<details>
<summary>Why do the arrays start with null values?</summary>

A moving average needs a full window before it can produce a value, so the
first `period − 1` entries are `null` for all three averages. The EMA's first
real value (at index `period − 1`) is seeded with the simple mean of that
first window, then advanced with the EMA recurrence from there.

</details>

<details>
<summary>SMA, EMA, or WMA — which one should I look at?</summary>

SMA weighs every point in the window equally, so it's the smoothest but the
slowest to react. EMA (smoothing factor `k = 2/(period + 1)`) responds faster
to recent moves — it's the usual choice in trading. WMA sits in between,
weighting the window linearly `1, 2, …, period` so the newest value counts
most. The tool returns all three so you can compare directly.

</details>

<details>
<summary>What are the size limits?</summary>

Up to 100,000 data points and a period of up to 10,000. The period must also
be no larger than the number of points you paste — a 20-point average of a
10-value series is rejected with an error rather than padded.

</details>

<details>
<summary>How precise are the results?</summary>

Each value is rounded to 6 decimal places before it's returned. That's enough
for price and sensor data while keeping the output readable; the underlying
computation itself runs in full 64-bit floating point.

</details>
