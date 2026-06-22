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
