## Validate an IBAN in your browser

Paste an IBAN (International Bank Account Number) and instantly see whether it's
valid. The check runs entirely in your browser — the number is never uploaded.

### What you get

- **Valid / invalid** against the **ISO 7064 mod-97** checksum.
- A **country and length** check using the official IBAN registry — an IBAN that
  passes the checksum but is the wrong length for its country is flagged.
- The **country name and code**, the two **check digits**, and the **BBAN**
  (the country-specific part after the check digits).
- For many common countries (UK, Germany, France, Spain, Netherlands, Italy,
  Belgium, Switzerland, Austria, Ireland), the **bank code** and **account
  number** parsed out of the BBAN.
- A nicely **4-character-grouped** version for display.

### Notes

- Spaces are ignored and letters are upper-cased, so you can paste
  `GB82 WEST 1234 5698 7654 32` or `gb82west12345698765432`.
- A valid checksum means the IBAN is **well-formed**, not that the account
  exists or is open — it's a structural check, not a bank lookup.

## FAQ

<details>
<summary>If the IBAN is "valid", does the bank account exist?</summary>

No. Valid means the number is structurally correct: right length for its country
(per the SWIFT IBAN registry) and a passing ISO 7064 mod-97 checksum. Whether the
account is real, open, or belongs to who you think requires a bank or payment
provider — no offline tool can tell you that.

</details>

<details>
<summary>Why is my IBAN rejected even though the check digits are right?</summary>

Usually the length. Every country has a fixed IBAN length — 22 for GB and DE, 18
for NL, 27 for FR, and so on — and the validator flags any mismatch even when the
mod-97 checksum happens to pass. A missing or extra character is the most common
cause; the reported length helps you spot it.

</details>

<details>
<summary>Do spaces, dashes, or lowercase letters matter?</summary>

Spaces are stripped and letters are upper-cased before validation, so
`gb82 west 1234 5698 7654 32` validates the same as the compact form. The result
also includes a normalized version and a display version grouped in blocks of 4.

</details>

<details>
<summary>For which countries can it split out the bank code and account number?</summary>

The BBAN layout is country-specific, so the breakdown is only shown where the
structure is unambiguous — currently the UK, Germany, France, Spain, the
Netherlands, Italy, Belgium, Switzerland, Austria, and Ireland. Other countries
still get the full validation, country name, check digits, and BBAN.

</details>
