## About this tool

The **MACD calculator** computes the *Moving Average Convergence Divergence* — one
of the most widely used momentum indicators in technical analysis — from any price
series. It returns all three MACD components at every point:

- **MACD line** — the difference between a fast and a slow exponential moving
  average of the price: `MACD = fastEMA − slowEMA`. The classic settings are a
  **12-period** fast EMA and a **26-period** slow EMA.
- **Signal line** — an exponential moving average of the MACD line itself, by
  default a **9-period** EMA. Crossovers between the MACD line and the signal line
  are the indicator's primary buy/sell triggers.
- **Histogram** — the gap between the MACD line and the signal line
  (`histogram = MACD − signal`). It visualises momentum: bars growing above zero
  show strengthening upward momentum, bars below zero the opposite.

Each EMA is seeded with the simple mean of its first window (the standard finance
convention), then advanced with the smoothing factor `k = 2 / (period + 1)`.
Values are reported as `null` during the *warm-up* region — before enough data
points are available to fill the window.

## How to use it

1. Paste or type your **price series** (separated by spaces, commas, semicolons, or
   newlines), oldest value first.
2. Optionally adjust the **fast**, **slow**, and **signal** periods — the defaults
   are the standard 12, 26, and 9.
3. Read off the MACD line, signal line, and histogram arrays — one entry per input
   point — plus the latest value of each.

The fast period must be smaller than the slow period, and the slow period must not
exceed the number of data points. To fully warm up the signal line you need at
least `slow + signal − 1` points (34 with the default settings).

## How MACD is read

- **MACD line crossing above the signal line** is a common bullish signal; crossing
  below is bearish.
- **The histogram** flips sign at those crossovers and shows their momentum.
- **MACD crossing the zero line** marks the fast EMA crossing the slow EMA.

## Common uses

- Spotting momentum shifts and trend changes in **stock, crypto, or forex** prices.
- Confirming entries/exits alongside other indicators in **technical analysis**.
- Smoothing and analysing any sequential numeric series where convergence and
  divergence of trends matter.

## Privacy

Everything runs locally in your browser via WebAssembly. Your data is never
uploaded to a server.

## FAQ

<details>
<summary>Why are the first values in each series null?</summary>

That's the warm-up region. Each EMA is seeded with the simple mean of its
first window, so the MACD line only starts at the slow period's point (26 by
default), and the signal line needs a further `signal` MACD values — a full
warm-up takes `slow + signal − 1` points (34 with the default 12/26/9).

</details>

<details>
<summary>How much data can I paste, and are the periods capped?</summary>

Up to 100,000 data points and periods up to 10,000. Two other rules apply: the
fast period must be smaller than the slow period, and the slow period can't
exceed the number of points you supply — otherwise you get a clear error
instead of a partial result.

</details>

<details>
<summary>Why don't my numbers exactly match TradingView or my broker?</summary>

Check the seeding. This tool uses the standard finance convention — each EMA
starts from the simple average of its first window, then advances with
`k = 2/(period + 1)`. Platforms that seed the EMA from the very first price
instead will differ slightly at the start of the series; the two converge as
more data arrives.

</details>

<details>
<summary>What order and format should the prices be in?</summary>

Oldest first, newest last — MACD is a running computation, so a reversed
series gives meaningless output. Values can be separated by spaces, commas,
semicolons, or newlines, and every entry must be a finite number.

</details>
