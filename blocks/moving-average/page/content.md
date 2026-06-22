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
