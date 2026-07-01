## Generate a Luhn check digit in your browser

Paste a **partial number** — the payload *without* its check digit — and instantly
get the Luhn (mod-10) check digit plus the completed, valid number. Everything runs
locally in your browser; the number is never uploaded.

### What you get

- The single **check digit** (0–9) that makes the number pass the Luhn check.
- The **full number**: your payload with the check digit appended.
- The length of the completed number.

### Check digit vs. validation

This tool **generates** the missing check digit — every digit you enter is treated
as payload. If you instead have a *complete* number and want to know whether it's
already valid, use the [Luhn Validator](/tools/luhn-validate/), where the last digit
is treated as the existing check digit.

### Notes

- Spaces and dashes are ignored, so you can paste `4242 4242 4242 424`.
- The Luhn algorithm is the checksum used by credit/debit cards, IMEI numbers, and
  many ID schemes. A passing check digit only catches accidental typos — it does
  **not** mean a card is real or active.

## FAQ

<details>
<summary>Do I include the last digit of my number when pasting?</summary>

No — paste the payload *without* its check digit. This tool treats every digit
you enter as payload and appends the missing mod-10 digit. If you paste a full
15-digit card body, you get back the 16th digit and the completed number. To test
a number that already has its check digit, use the Luhn Validator instead.

</details>

<details>
<summary>What characters are allowed in the input?</summary>

Digits, spaces, and dashes — separators are stripped before computing, so
`4242-4242-4242-424` and `4242 4242 4242 424` both work. Any other character
(letters, dots, etc.) produces an explicit error rather than being silently
dropped, and an input with no digits at all is rejected.

</details>

<details>
<summary>Does a valid Luhn check digit mean the card number is real?</summary>

No. Luhn is only a typo detector — it catches single-digit errors and most
adjacent transpositions. Any random digit string can be completed to a
Luhn-valid number, so a passing checksum says nothing about whether a card
account exists, is active, or belongs to anyone.

</details>

<details>
<summary>Where is Luhn used besides credit cards?</summary>

IMEI numbers on phones, Canadian Social Insurance Numbers, many national ID and
loyalty-card schemes, and various invoice/reference-number formats all append a
Luhn mod-10 check digit — this tool works for any of them since the algorithm is
identical regardless of length.

</details>
