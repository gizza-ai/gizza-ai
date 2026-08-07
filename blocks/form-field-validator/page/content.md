## About this tool

Use this form field validator when you need a quick, offline check of a whole submitted form rather than one value at a time. Paste `name: value` lines (or a JSON object), choose a country, mark required fields, and get a per-field report for common input types: email, phone, URL, postal code, credit card, and plain text.

Field types are inferred from names such as `email`, `phone`, `zip`, `postal_code`, `website`, `card`, and `cc_number`. Add rules like `zip: postal-code` or `support_url: url` when a field name is ambiguous. Country selection controls postal-code patterns and phone calling-code / national-length checks. The tool can normalize passing values and masks credit-card numbers by default.

This is a deterministic format validator. It does not perform DNS or MX checks, phone carrier lookups, address-existence checks, or BIN/issuer lookups. A passing result means the value is well formed for the selected rule; it does not prove the mailbox, phone number, address, or card account exists.

## Worked examples

Validate a US signup form:

```text
fields:
email: John.Doe@Example.COM
phone: (415) 555-2671
zip: 90210
website: https://example.com
card: 4111 1111 1111 1111

country: US
required: email, phone, zip
rules:
zip: postal-code
card: credit-card
```

The report starts with `VALID` when every required and typed field passes, shows normalized forms such as a lower-cased email domain and an E.164-style phone number, and masks card digits except the last four.

Find every issue in a broken form:

```text
fields:
email: john@
phone: 555-12
zip: 9021

country: US
required: email, phone, zip, website
rules: zip: postal-code
```

The output lists each failing field with the reason, expected format, and an example for the selected country.

## Supported checks and limits

- Inputs: up to 200 fields, as `name: value` lines or a JSON object.
- Field types: `email`, `phone`, `url`, `postal-code`, `credit-card`, and `text`.
- Required fields: blank, `*`, comma-separated names, or one name per line.
- Countries: `any` plus 38 country codes for postal-code and phone rules.
- Credit cards: length, brand prefix, and Luhn checksum only; no issuer lookup.
- Network checks: none. Everything runs locally and deterministically.

## FAQ

<details>
<summary>Does a valid email result mean the mailbox can receive mail?</summary>

No. This checks offline email syntax: one `@`, legal local/domain characters, label lengths, and a plausible alphabetic top-level domain. It does not query DNS, MX records, disposable-domain lists, or SMTP servers.

</details>

<details>
<summary>How are phone numbers validated?</summary>

With `country = any`, the phone check uses a generic E.164-style digit window. With a country code such as `US`, `GB`, or `DE`, it checks the calling code, accepted national digit length, and common trunk prefix handling. It is not a carrier-grade phone-number database or line-type lookup.

</details>

<details>
<summary>Can I validate postal codes for every country?</summary>

The tool includes a practical table of common postal-code formats for 38 countries and a generic `any` fallback. If a country has complex delivery-point rules, this tool checks the printed format only; it does not verify that a code is currently assigned or deliverable.

</details>

<details>
<summary>Why are credit-card numbers masked?</summary>

Masking is on by default so reports, screenshots, and logs do not expose full card numbers. The tool still uses the full input for brand and Luhn checks, then displays only the final four digits. Turn off `mask_sensitive` only when you explicitly need to inspect the exact normalized digits.

</details>
