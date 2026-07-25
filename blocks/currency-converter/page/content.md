## About the currency converter

This tool converts an amount from one currency to another using an
**exchange-rate table you supply** — it does not fetch live rates. That makes it
useful when you want to convert against a specific, known rate: the rate printed
on a receipt, a rate agreed for an invoice, a historical rate, an internal
accounting rate, or a crypto price you already have.

Fill in the **amount**, the **from** and **to** currency codes, and paste your
**rates**, one per line. Each rate line is `FROM TO RATE`, meaning *1 FROM =
RATE TO*:

```
USD EUR 0.92
EUR JPY 160
```

With that table, converting **100 USD → EUR** gives:

```
100 USD = 92.00 EUR
Rate: 1 USD = 0.92 EUR
```

### Chained (multi-leg) conversions

You do not need a direct rate between every pair. If the table connects the two
currencies through intermediates, the converter follows the path with the
**fewest legs**. With the two lines above, converting **100 USD → JPY** works
even though there is no `USD JPY` line:

```
100 USD = 14,400.00 JPY
Rate: 1 USD = 144 JPY
Path: USD → EUR → JPY (2 legs)
```

When a direct rate *and* a longer route both exist, the direct rate wins.

### Reverse rates

By default every rate is also usable in reverse (`1/rate`), so a single
`USD EUR 0.92` line converts **both** USD→EUR and EUR→USD. Untick **Use each
rate in reverse** to only follow rates in the exact direction you wrote them.

### Rate-table format

- One rate per line: two currency codes and one number, in `FROM TO RATE` order.
- Codes are 2–10 letters/digits and case-insensitive (`usd` = `USD`), so
  ISO codes (`USD`, `EUR`, `JPY`) and crypto tickers (`BTC`, `ETH`) both work.
- The code/number separators are flexible — space, `/`, `:`, `=`, or `,` all
  work, so `USD/EUR = 0.92` parses the same as `USD EUR 0.92`.
- Blank lines and anything after a `#` are ignored, so you can comment your table.
- The **number can sit in any position** on the line; the two codes keep their
  `FROM TO` order (`USD 0.92 EUR` is read as *1 USD = 0.92 EUR*).

Everything runs locally in your browser. Your amounts and rates are never
uploaded, and the tool works offline.

### FAQ

<details>
<summary>Does this use live, up-to-the-minute exchange rates?</summary>

No. It converts strictly against the rate table you paste, so the result is only as current as the rates you enter. This is deliberate — it lets you convert against a specific receipt, invoice, historical, or internal rate rather than whatever a live feed happens to show.

</details>

<details>
<summary>How do I convert between two currencies with no direct rate?</summary>

Include rates that connect them through an intermediate currency. If your table has `USD EUR 0.9` and `EUR JPY 160`, converting USD→JPY chains the two automatically and reports the path used (`USD → EUR → JPY`). It always takes the route with the fewest legs.

</details>

<details>
<summary>What does the "reverse rate" option do?</summary>

When it is on (the default), each rate line also works backwards using `1/rate`, so one `USD EUR 0.92` line handles both directions. Turn it off if your rates are directional (for example a buy rate that should not be inverted into a sell rate).

</details>

<details>
<summary>Can I convert cryptocurrencies?</summary>

Yes. Currency codes are just 2–10 letter/digit tokens, so `BTC`, `ETH`, or any ticker works exactly like `USD` or `EUR`. Increase the decimal places for small crypto amounts.

</details>

<details>
<summary>Why is my conversion failing?</summary>

The two most common causes are a rate line that is not in `FROM TO RATE` form (each line needs exactly two codes and one positive number) and no path connecting the two currencies in your table. The error message points at the offending line or the missing link.

</details>
