## About this tool

A check digit is one extra character appended to a code so a single mistyped or transposed digit can be caught before the code is used. Almost every identifier you handle carries one: payment cards and IMEIs use Luhn, retail barcodes and ISBN-13 use the GS1 mod-10 weighting, ISBN-10 and ISSN use mod-11 (which is why an `X` can appear), IBANs use mod-97-10, and VINs use a mod-11 transliteration with the check character in the middle rather than at the end.

This validator recognises all of them. Leave the scheme on **auto-detect** and it works out every scheme the code's length and character set allow, tells you which ones the code actually passes, and adds context such as the card brand, IBAN country, ABA Federal Reserve routing symbol, or VIN world manufacturer identifier. Pick a scheme explicitly when you want a strict answer — an ISBN-13 that fails is then reported as a failed ISBN-13 rather than quietly re-detected as something else.

**Compute mode** goes the other way: give it the payload *without* a check digit and it returns the digit plus the completed code. That is the fastest way to finish a barcode you are minting, generate a test card number, or derive IBAN check digits from a country code and account number.

Spaces, dashes, dots, slashes and colons are ignored, so codes copied from an invoice, a spreadsheet or a barcode label work as-is. Paste a whole list — one code per line, or separated by commas or semicolons — and every code is checked in one pass with a valid/invalid tally at the top.

### Worked example

Input, with the scheme on auto-detect:

```text
4539 1488 0343 6467
978-0-306-40615-7
GB82 WEST 1234 5698 7654 32
021000021
```

Output:

```text
Checked 4 codes — 4 valid, 0 invalid.

4539 1488 0343 6467
  VALID — Credit card (Luhn mod-10) · Visa
  also valid as: Luhn (mod-10)

978-0-306-40615-7
  VALID — ISBN-13 (GS1 mod-10)
  also valid as: EAN-13 / GTIN-13

GB82 WEST 1234 5698 7654 32
  VALID — IBAN (mod-97-10) · United Kingdom, 22 characters, BBAN WEST12345698765432

021000021
  VALID — ABA routing number · Federal Reserve routing symbol 0210
```

When a code fails, the report names the digit it should have carried, which usually points straight at the typo:

```text
9780306406158
  INVALID — ISBN-13 (GS1 mod-10)
  expected check digit 7, got 8
```

Turn on **show the arithmetic** to see the weighted sum and modulo step behind that verdict:

```text
  steps: GS1 weighted sum (×3/×1 from the right, check digit excluded) = 93; (10 − 93 mod 10) mod 10 = 7
```

## Supported schemes

| Scheme | Length | Algorithm |
| --- | --- | --- |
| Luhn, payment card, IMEI, NPI | any / 15 / 10 | mod-10, doubling every second digit from the right |
| EAN-8, UPC-A, EAN-13, ISBN-13, GTIN-14, SSCC | 8 / 12 / 13 / 14 / 18 | GS1 mod-10, ×3/×1 weights from the right |
| ISBN-10, ISSN | 10 / 8 | mod-11, check character may be `X` |
| ABA routing number | 9 | weighted 3-7-1 mod-10 |
| IBAN | 15–34, per country | mod-97-10 on the rearranged, transliterated code |
| ISIN | 12 | letters expanded to digits, then Luhn |
| VIN | 17 | mod-11 transliteration, check character at position 9 |

## Limits and edge cases

- Up to 5,000 codes per run. Split larger lists into batches.
- A check digit only detects errors; it does not prove a code was ever issued. A valid Luhn number is not necessarily a real account, and a valid ISBN-13 is not necessarily a published book.
- Auto-detect deliberately reports multiple matches. Every ISBN-13 is also a structurally valid EAN-13, and many 9-digit routing numbers happen to satisfy Luhn as well.
- IBAN length is checked against the country's published length, so a code with the right check digits but the wrong length is still reported invalid.
- Compute mode requires an explicit scheme — auto-detect reads the check digit that compute mode is meant to produce.
- For VIN, compute mode takes the full 17 characters and rewrites position 9. For IBAN, pass the country code plus the account part; existing check digits are replaced.
- Everything runs in your browser. Codes are never uploaded.

## FAQ

<details>
<summary>What is the difference between validate and compute mode?</summary>

Validate mode assumes the code already ends with its check digit and tells you whether that digit is correct. Compute mode assumes the digit is missing: you pass the payload, and it returns the digit plus the completed code. Compute mode needs an explicit scheme because auto-detection works by reading the very digit you are asking it to generate.

</details>

<details>
<summary>Why does one code match several schemes at once?</summary>

Many schemes share an algorithm and differ only in length or prefix. An ISBN-13 is a GS1 mod-10 code of 13 digits, which is exactly the definition of an EAN-13, so a valid ISBN-13 is always a valid EAN-13. Auto-detect lists every scheme the code passes under `also valid as:` instead of picking one and hiding the rest. Choose the scheme explicitly if you want a single strict verdict.

</details>

<details>
<summary>Does a valid check digit mean the number is real?</summary>

No. Check digits catch keying errors — a single wrong digit, and most transpositions of adjacent digits. They say nothing about whether a card was issued, an account exists, or a book was published. Use this to reject typos early; use the issuing system to confirm the code is live.

</details>

<details>
<summary>Why can an ISBN-10 or ISSN end in X?</summary>

Both use mod-11, so the check value ranges from 0 to 10. Ten needs a single character, and the convention is the letter `X`. ISBN-13 moved to the GS1 mod-10 algorithm, where the check value is always 0–9, which is why modern book barcodes never end in `X`.

</details>

<details>
<summary>Can I check a whole list at once?</summary>

Yes. Put one code per line, or separate them with commas or semicolons, up to 5,000 per run. The report opens with a valid/invalid/unreadable tally and then gives a verdict per code, so a spreadsheet column can be pasted in directly. Spaces, dashes, dots, slashes and colons inside a code are ignored.

</details>

<details>
<summary>Is it safe to paste a real card number or IBAN?</summary>

The calculation runs entirely in your browser via WebAssembly — nothing is sent to a server and nothing is stored. That said, a check digit only needs the digits themselves, so test values such as `4539 1488 0343 6467` are enough to see how the tool behaves before you use production data.

</details>
