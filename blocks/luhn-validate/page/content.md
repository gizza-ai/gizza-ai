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
