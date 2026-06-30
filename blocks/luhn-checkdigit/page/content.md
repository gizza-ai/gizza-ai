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
