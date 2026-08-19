## Format money without changing currencies

Use this formatter when you already have a number and need a clean currency string
for a table, invoice, spreadsheet export, fixture, README, or test case. It does
not fetch exchange rates and it does not convert USD to EUR. It only formats the
amount you paste: currency marker, placement, separators, decimal places, rounding
and sign style.

### Worked example

For a European-style EUR display, use the **European EUR** preset or set:

- Number: `1234.5`
- Currency code or symbol: `EUR`
- Currency display: `code`
- Currency position: `after`
- Space between currency and number: on
- Decimal places: `2`
- Group separator: `period`
- Decimal separator: `comma`

The result is `1.234,50 EUR`.

### What the controls mean

**Currency display** chooses a known symbol (`USD` → `$`), the code itself (`USD`),
or no currency marker. **Decimal places** accepts `0` through `8` and rounds the
pasted digit string directly, so money cases such as `1.005` round predictably.
**Digit grouping** supports western thousands (`1,234,567`) and Indian lakh/crore
(`12,34,567`). **Sign style** covers normal minus signs, explicit plus signs,
aligned leading spaces, absolute values, and accounting parentheses.

### Input rules and limits

The input can include common copied formatting: spaces, underscores, apostrophes,
commas or periods as grouping marks, currency affixes such as `$1,234.50`,
negative parentheses such as `(1234.50)`, a trailing minus such as `1234.50-`, or
scientific notation such as `1.5e3`. If both `.` and `,` appear, the last one is
read as the decimal mark. A lone comma with exactly three digits after it is read
as grouping (`1,234`); otherwise it is a decimal comma (`0,5`). Outputs are plain
text and there is no locale database or exchange-rate lookup.

## FAQ

<details>
<summary>Does this convert between currencies?</summary>

No. It is presentation-only. `USD`, `EUR`, `INR` and other codes only choose the
text or symbol that appears next to the number; the numeric amount is not changed.

</details>

<details>
<summary>Why does 1.005 round to 1.01 here?</summary>

The core rounds the decimal digits you typed rather than first converting them to
a binary floating-point number. That avoids the common money-formatting surprise
where `1.005` becomes `1.00` after naive float rounding.

</details>

<details>
<summary>Can I format Indian numbering such as lakhs and crores?</summary>

Yes. Set **Digit grouping style** to `indian`; `12345678` formats as
`1,23,45,678` before the currency marker and fraction controls are applied.

</details>

<details>
<summary>When should I use accounting parentheses?</summary>

Use them for reports where negative amounts are shown as `($1,234.50)` instead of
`-$1,234.50`. The option affects only negative values; zero and positives remain
unwrapped.

</details>
