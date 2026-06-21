## About this tool

**Bollinger Bands** are a volatility indicator built from a moving average and
the standard deviation of a price series:

- **Middle band** — the simple moving average (SMA) over the last `period` values
  (commonly 20).
- **Upper / lower bands** — the middle band plus and minus `num_std` standard
  deviations (commonly 2). The bands widen when the market is volatile and
  contract when it is calm.

For the most recent point the tool also reports:

- **%B** — where the price sits within the bands: `(price − lower) / (upper −
  lower)`. 0 means the price is on the lower band, 1 means it is on the upper band.
- **Bandwidth** — `(upper − lower) / middle`, a measure of how wide the bands are.

Enter the prices oldest-first, separated by spaces, commas, semicolons, or
newlines. The standard deviation is the **population** standard deviation (÷N) of
each window, matching the classic Bollinger definition.

### Privacy

Everything runs **in your browser** via WebAssembly — your data is never uploaded.
Also available from the [gizza CLI](/) and in chat (which return the bands as
structured JSON).
