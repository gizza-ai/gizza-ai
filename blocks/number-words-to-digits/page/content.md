## Turn spelled-out numbers into digits

Paste anything that contains numbers written in English words — a sentence, a
transcript, a column of phrases from a spreadsheet — and get the digits back.
The default mode rewrites the text in place and leaves every other word alone,
so a paragraph stays a paragraph. Everything runs in your browser; the text you
paste is never uploaded.

### Worked example

Using the **Numbers in prose** preset:

- Text or number words: `We shipped twenty-five units and one hundred and two spares.`
- What to return: `replace`
- Thousands separator: `none`
- Ordinal words: `cardinal`

The result is:

`We shipped 25 units and 102 spares.`

Switch **What to return** to `value`, paste `one million two hundred fifty
thousand`, and pick the comma separator to get `1,250,000` instead.

### What the controls mean

**What to return** picks the shape of the output. `replace` (default) rewrites
numbers where they appear and keeps the surrounding prose. `value` treats every
non-empty line as one complete number phrase and reports an error naming the
first word that does not belong — useful for cleaning a column of data.
`extract` throws the prose away and returns just the numbers, one per line.

**Thousands separator** only affects how digits are rendered: `none` keeps the
output machine-readable, while comma, space and underscore group by threes.

**Billion / trillion reading** switches between the short scale (a billion is
10⁹) and the long scale used in much of continental Europe (a billion is 10¹²,
a trillion 10¹⁸). `milliard` is always 10⁹ in both, and the Indian `lakh`
(100,000) and `crore` (10,000,000) are always accepted.

**Ordinal words** decides what `twenty-first` becomes: `21`, `21st`, or nothing
at all. **Read half and quarter as fractions** turns `one and a half` into
`1.5`; turn it off when those words are ordinary English in your text. **Read
digit runs** is off by default because it changes meaning: with it on,
`one two three` becomes `123` rather than `1 2 3`.

### What it understands

Units, teens and tens (`seven`, `nineteen`, `forty`), hyphenated compounds
(`twenty-five`), `hundred` and scale words up to `decillion`, the connector
`and` only where English really uses it (`one hundred and forty-two`, but
`one and two` stays two numbers), decimal words (`five point forty-seven` →
`5.47`, `one point five million` → `1500000`), fraction words (`three quarters`
→ `0.75`, `half a million` → `500000`), and negatives (`minus one hundred`,
`negative twenty one`).

### Limits and edge cases

- Input is capped at **200,000 characters**; longer text is rejected with a clear message.
- Arithmetic is exact 128-bit decimal, so values must fit in roughly **±1.7 × 10³⁸**. `one decillion` (10³³) works; a long-scale `one septillion` (10⁴²) reports "number is too large" rather than losing precision to floating point.
- Only `half` and `quarter` are treated as fractions — `third`, `fifth` and `eighth` are also ordinal words, and thirds have no exact decimal form.
- English only. Roman numerals, currency phrases such as `one dollar and fifty cents`, and multiplicative phrases such as `six sixes` are not parsed.
- `value` mode is deliberately strict: a stray word on a line is an error, not a silent skip. Use `extract` or `replace` for messy text.

## FAQ

<details>
<summary>Why did "one and two" not become 12 or 1.2?</summary>

`and` is only treated as part of a number where English actually uses it — after
`hundred` or a scale word (`one hundred and forty-two`, `one thousand and one`),
or before a fraction (`one and a half`). Everywhere else it stays an ordinary
conjunction, so `one and two` converts to `1 and 2`.

</details>

<details>
<summary>How do I convert a whole column of phrases at once?</summary>

Paste one phrase per line and set **What to return** to `value`. Each line is
converted independently and the results come back in the same order, so you can
paste the output straight back into a spreadsheet. Blank lines are skipped.

</details>

<details>
<summary>Does it understand lakh and crore?</summary>

Yes. `three lakh` is `300000` and `two crore fifty lakh` is `25000000`, in both
the short-scale and long-scale settings — those words never collide with
`million`/`billion`. Indian *output* grouping (`12,34,567`) is not offered here;
the separator setting groups by threes.

</details>

<details>
<summary>What happens to ordinals like "twenty-first"?</summary>

With the default `cardinal` setting it becomes `21`. Choose `suffix` for `21st`,
which is what you usually want when rewriting dates in prose. Choose `ignore` to
stop ordinal words ending a number phrase — note that in replace mode
`twenty-first` then shows as `20-first`, because only the cardinal part converts.

</details>

<details>
<summary>Is my text sent anywhere?</summary>

No. The converter is compiled to WebAssembly and runs entirely in your browser —
there is no server call, no logging and no network access of any kind. The same
code is available offline through the command line.

</details>
