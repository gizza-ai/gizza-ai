## About this tool

Writing a number out in words is one of those chores that only looks simple. The digits are easy; the conventions are not. Is it "one thousand two hundred thirty-four" or "one thousand two hundred **and** thirty-four"? Does a cheque want "twenty-five dollars and forty cents" or "twenty-five and 40/100 dollars"? Is 10⁹ a billion or a milliard? Is 125000 a hundred and twenty-five thousand or one lakh twenty-five thousand?

This tool answers all of those with one setting each. Pick a style, a scale, a letter case and a currency, paste your number, and the words come back in the convention you actually need — for a contract, an invoice, a cheque, a legal amount in words, a voice-over script, or a spelled-out figure in copy.

Numbers are handled as digit strings from start to finish, never as floating-point values. That matters in two places: very large integers spell exactly rather than drifting in the last few digits, and a money amount like `1.005` rounds the way an accountant expects rather than the way a binary float happens to fall.

### Worked example

Input `1234` with the default settings returns:

```text
one thousand two hundred thirty-four
```

Turn on **British 'and'** and it becomes:

```text
one thousand two hundred and thirty-four
```

Set the style to **Cheque**, the case to **Sentence case**, and tick **Append 'only'**, then paste `14,273.38`:

```text
Fourteen thousand two hundred seventy-three and 38/100 dollars only
```

Switch the scale to **Indian**, the currency to **INR** and the style to **Money**, and `125000` reads:

```text
one lakh twenty-five thousand rupees
```

### Batches

Paste one number per line and you get one result per line, in order — handy for a column copied out of a spreadsheet:

```text
1
2
103
```

with the style set to **Ordinal digits** returns:

```text
1st
2nd
103rd
```

Blank lines are skipped. If one line cannot be read, the error names the line number so you can find it in a long paste.

### What the input accepts

A number does not have to be typed cleanly. All of these parse to the same value:

- `1234`, `1,234`, `1 234`, `1_234`, `1'234` — thousands separators in any common form
- `1234,5` — a comma used as the decimal point
- `$1,234`, `£1 234`, `₹1,25,000` — currency symbols are ignored, including Indian digit grouping
- `-12`, `minus 12`, `negative 12`, `(12)` — all negative twelve, including the accounting parentheses form
- `1.5e3`, `2.5E-4` — scientific notation, expanded before spelling

## Options and limits

- **Styles**: cardinal words, ordinal words (`twenty-first`), ordinal digits (`21st`), spoken year (`nineteen eighty-four`, `nineteen oh five`, `two thousand five`), money, and the cheque/invoice form with the fraction over 100.
- **Scales**: short (10⁹ = one billion), long (10⁹ = one milliard, 10¹² = one billion), and Indian (thousand, lakh, crore, arab, kharab and up to mahashankh).
- **Currencies**: 25 codes — USD, EUR, GBP, JPY, INR, CAD, AUD, CHF, CNY, RUB, BRL, MXN, ZAR, NGN, KRW, NZD, SGD, HKD, SEK, NOK, DKK, PLN, TRY, PHP and THB — each with its own unit and sub-unit words. JPY and KRW have no sub-unit, so amounts are rounded to whole units.
- **Decimals**: read digit by digit after "point", truncated, or rounded half away from zero. Money and cheque styles always round to the currency's sub-unit instead.
- **Case**: lower, UPPER, Title, or Sentence.
- Size limits: 66 digits on the short and long scales (up to vigintillion / undecilliard), 21 digits on the Indian scale (up to mahashankh), 30 fraction digits, 1,000 numbers and 100,000 bytes per run.
- Everything runs in your browser. Nothing you paste is uploaded.

## FAQ

<details>
<summary>Should I use "and" — one thousand and twenty-three, or one thousand twenty-three?</summary>

Both are correct; they are regional. American English usually omits the "and", British and Commonwealth English usually keeps it before the final group under one hundred. The **British 'and'** checkbox switches between them, and it applies inside money amounts too. For legal or cheque wording, follow whatever your bank or template already uses.

</details>

<details>
<summary>What is the difference between the Money and Cheque styles?</summary>

The Money style spells both halves of the amount: `25.40` becomes "twenty-five dollars and forty cents". The Cheque style writes the sub-unit as a fraction over 100, which is the convention on a cheque's amount line: "twenty-five and 40/100 dollars". A whole amount still gets the fraction, so `500` becomes "five hundred and 00/100 dollars" — that zero fraction is what stops someone adding cents after the fact. Tick **Append 'only'** to add the usual terminator.

</details>

<details>
<summary>Why does a billion change when I switch the scale?</summary>

Because there are two conventions. On the short scale, used in the US and in modern British usage, each new name is a thousand times the last, so 10⁹ is a billion. On the long scale, still used across much of continental Europe, each new name is a million times the last and the intermediate steps are named milliard, billiard and so on — 10⁹ is a milliard and 10¹² is a billion. The Indian scale is different again: after a thousand it groups in two-digit steps as lakh (10⁵), crore (10⁷), arab (10⁹) and upward.

</details>

<details>
<summary>How are decimals handled?</summary>

On the word styles, the default reads the fraction digit by digit after the word "point", so `1.50` is "one point five zero" — trailing zeros are kept because they are what you typed. Choose *Drop it* to truncate toward zero, or *Round it* to round half away from zero to a whole number. The Money and Cheque styles ignore that setting and round to the currency's sub-unit, which is why `1.005` comes back as "one dollar and one cent".

</details>

<details>
<summary>Can it handle very large numbers exactly?</summary>

Yes, within its named-scale range. Digits are never converted to a floating-point number, so a 66-digit integer spells out digit-accurately rather than being rounded to the nearest representable double. Past 66 digits on the short and long scales — or 21 on the Indian scale — there is no agreed name for the next group, so the tool reports the limit instead of inventing one.

</details>

<details>
<summary>Does the Year style work for every number?</summary>

It works the way English speakers read four-digit years: `1984` is "nineteen eighty-four", `1905` is "nineteen oh five", `1900` is "nineteen hundred". Years in the x000–x009 block read as a whole number instead, so `2005` is "two thousand five", which matches how people actually say it. Anything that is not four digits falls back to the ordinary cardinal reading, and a negative year gets "BC" appended.

</details>
