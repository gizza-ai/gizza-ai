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

## FAQ

<details>
<summary>Why do I get fewer band rows than input values?</summary>

A band needs a full window of `period` values, so the first `period − 1`
points have no band. With `N` prices and a period of `P` you get `N − P + 1`
rows, aligned to the input by their `index`. If you supply fewer than `period`
values the tool returns an error telling you the minimum required.

</details>

<details>
<summary>Does this use the sample or the population standard deviation?</summary>

The **population** standard deviation (divide by N), which is the classic
Bollinger Bands definition. Spreadsheet functions like `STDEV`/`STDEV.S` use
the *sample* deviation (divide by N−1), so their bands come out slightly wider —
compare against `STDEV.P` instead if you're cross-checking.

</details>

<details>
<summary>What do %B and bandwidth tell me, and when are they missing?</summary>

Both describe the most recent point. **%B** is `(price − lower) / (upper −
lower)`: 0 means the price sits on the lower band, 1 on the upper band, and
values outside 0–1 mean it closed outside the bands. **Bandwidth** is
`(upper − lower) / middle`. %B is omitted when the bands have zero width (a
constant series), and bandwidth is omitted when the middle band is 0.

</details>

<details>
<summary>How should I format the price list?</summary>

Paste the values **oldest first**, separated by spaces, commas, semicolons, or
newlines — a column copied from a spreadsheet works as-is. Every token must be
a finite number; anything else (like a header cell) is reported back as
`'…' is not a number`.

</details>
