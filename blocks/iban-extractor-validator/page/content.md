## About this tool

**Extract & Validate IBANs** scans pasted text — an invoice, an email, a bank
statement, a log — and pulls out every **IBAN** it contains, checking each one
with the official **ISO 13616 mod-97 checksum** and the country-specific length.

- **Finds IBANs in messy text**: grouped in the usual 4-character blocks
  (`GB82 WEST 1234 5698 7654 32`) or written contiguously
  (`DE89370400440532013000`), with surrounding punctuation stripped.
- **Validated, not just matched**: each candidate is sized to its country's
  registered IBAN length and run through the mod-97 (ISO 7064) checksum, so a
  single mistyped character is caught.
- **Split into valid and invalid**: correct IBANs are listed with their country
  and 4-block formatting; structurally IBAN-shaped strings that fail the checksum
  are listed separately so you can spot typos.
- **Deduplicated** in first-seen order — the same IBAN written two ways
  (spaced and unspaced) counts once.

Everything runs **locally in your browser** via WebAssembly — the text you paste
is never uploaded.

### Worked example

Paste:

```
Please remit payment to GB82 WEST 1234 5698 7654 32. Our old account
DE89 3704 0044 0532 0130 00 is no longer in use.
```

Result:

```
Found 2 IBAN(s): 2 valid, 0 invalid.

Valid (2):
  GB82 WEST 1234 5698 7654 32  -  United Kingdom
  DE89 3704 0044 0532 0130 00  -  Germany
```

### Handy for

- Pulling the payee IBAN out of an invoice or email without retyping it.
- Sanity-checking a batch of pasted account numbers for typos before a transfer.
- Extracting a deduplicated IBAN list from a statement or export.

### Limits & edge cases

- Detects IBANs for the ~78 countries in the SWIFT IBAN registry (a two-letter
  country code with no registered IBAN, e.g. `US`, is skipped).
- **Checksum, not existence.** Passing mod-97 proves an IBAN is well-formed for
  its country — it does **not** prove the account is open or that the bank still
  exists. Only the bank can confirm that.
- It reports the **country** and the bank/account components parsed from the
  BBAN, but does **not** look up the bank's name or BIC.

## FAQ

<details>
<summary>Does it work if the IBANs have no spaces, or unusual spacing?</summary>

Yes. Each IBAN is anchored at its two-letter country code plus two check digits
and read up to that country's exact registered length, tolerating the standard
4-character grouping spaces. So `DE89370400440532013000` and
`DE89 3704 0044 0532 0130 00` are both found — and recognized as the same IBAN.

</details>

<details>
<summary>What does "invalid" mean in the results?</summary>

An invalid entry is a string that looks like an IBAN — right country code, right
length — but fails the ISO 13616 mod-97 checksum. That almost always means a
mistyped or corrupted digit, so it's flagged separately instead of silently
dropped.

</details>

<details>
<summary>Does a valid IBAN mean the account exists?</summary>

No. The mod-97 checksum only proves the IBAN is mathematically well-formed for
its country. An IBAN can pass the check and still belong to a closed or
never-opened account — only the account's bank can confirm it actually exists.

</details>

<details>
<summary>Does it look up the bank name or BIC?</summary>

No. It reports the country and, for common countries, the bank code and account
number parsed out of the BBAN, but it does not resolve the bank's name or BIC —
that needs an external bank-directory lookup, which this in-browser tool doesn't
do.

</details>

<details>
<summary>Is my text uploaded anywhere?</summary>

No. The whole scan runs locally in your browser through WebAssembly. The text
you paste never leaves your device.

</details>
