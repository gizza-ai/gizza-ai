## What this tool does

Paste one value and validate it as an **email address, URL, IPv4/IPv6 address,
phone number, or credit-card number**. Leave the format on **auto** to try every
supported check and report the detected format, or choose a specific format when
you need a strict yes/no answer.

Everything runs locally in your browser. The checks are syntax and checksum
checks only: there are no DNS, MX, reachability, carrier, or payment-network
lookups.

## Supported checks

- **Email** — common internet email syntax: one `@`, sane local part, domain
  labels, and alphabetic top-level domain. It does not verify mailbox existence.
- **URL** — requires a scheme such as `https://` and a host; reports the scheme
  and host that were parsed.
- **IPv4 / IPv6 / IP** — uses strict address parsing for either address family.
- **Phone** — allows a leading `+`, spaces, hyphens, dots, and parentheses; must
  contain 7 to 15 digits.
- **Credit card** — accepts spaces or hyphens, checks 12 to 19 digits with the
  Luhn algorithm, and reports a best-effort card brand.

## Example

Input:

```text
4111 1111 1111 1111
```

Format: `credit-card`

Output:

```text
Input:    4111 1111 1111 1111
Format:   credit-card
Valid:    yes
Detail:   valid card number; Luhn OK; brand Visa; 16 digits
```

## Limits and edge cases

- This is a format validator, not an identity or availability check.
- Email validation intentionally does not perform DNS or MX lookups.
- URL validation requires `scheme://host`; a bare `example.com` is not treated as
  a valid URL.
- Phone validation is length and character based; it does not know national
  numbering plans.
- Test card numbers such as `4111 1111 1111 1111` can pass Luhn without being a
  real card.

## FAQ

<details>
<summary>Does this verify that an email inbox or URL actually exists?</summary>

No. It checks syntax only. There are no DNS, MX, HTTP, or reachability probes, so
private data stays local and the result does not depend on network access.

</details>

<details>
<summary>Why does a bare domain fail URL validation?</summary>

The URL check expects a full URL with a scheme and host, such as
`https://example.com`. A bare `example.com` can be a domain name, but it is not a
complete URL for this validator.

</details>

<details>
<summary>What does auto-detect do?</summary>

Auto-detect runs every supported check and reports the first valid match in a
stable priority order: IPv4, IPv6, email, URL, credit card, then phone. It also
prints a per-format pass/fail table so you can see ambiguous cases.

</details>

<details>
<summary>Is the credit-card check safe to use?</summary>

It performs only a local Luhn checksum and simple brand detection from the number
prefix and length. It does not contact a bank or payment network, and a Luhn-pass
number is not proof that a card exists or can be charged.

</details>
