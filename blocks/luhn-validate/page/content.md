## Luhn check in your browser

Paste a number — a credit/debit card, an IMEI, or any identifier that uses a
Luhn (mod-10) check digit — and instantly see whether it's valid. Everything
runs locally in your browser; the number is never uploaded.

### What you get

- **Valid / invalid** against the Luhn algorithm.
- The **correct last (check) digit** if it's invalid — handy for catching a
  single-digit typo or completing a number.
- A best-effort **card brand** (Visa, Mastercard, Amex, Discover, JCB, Diners)
  when the length and prefix match.

### Notes

- Spaces and dashes are ignored, so you can paste `4242 4242 4242 4242`.
- The Luhn check catches most accidental typos, but **passing it does not mean a
  card is real or active** — it's only a checksum.

## FAQ

<details>
<summary>If a number passes, does that mean the card is real?</summary>

No. Luhn is only a mod-10 checksum designed to catch typos — every issued
card passes it, but so do infinitely many made-up numbers. Whether an account
exists, is active, or has funds can only be verified by the card network,
which this tool never contacts.

</details>

<details>
<summary>What is the "expected check digit" it reports?</summary>

It's the last digit that *would* make your number pass the Luhn check. If a
number comes back invalid, comparing its actual last digit with the expected
one often pinpoints a single-digit typo — and if you're constructing a test
number, it tells you which final digit to append.

</details>

<details>
<summary>Which characters are allowed in the input?</summary>

Digits, spaces, and dashes only — so `4242 4242 4242 4242` and
`4242-4242-4242-4242` both work. Any other character stops the check with an
error naming it, and at least 2 digits are required.

</details>

<details>
<summary>Why doesn't it show a card brand for my number?</summary>

Brand detection is best-effort: it only reports Visa, Mastercard, Amex,
Discover, JCB, or Diners when both the prefix **and** length match that
scheme. IMEIs and other Luhn-checked identifiers aren't cards, so they
validate fine but show no brand.

</details>
