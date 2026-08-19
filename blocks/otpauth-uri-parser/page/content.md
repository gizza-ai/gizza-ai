## About this tool

A two-factor setup QR code is just a picture of a text link. Decode the QR and you get an
**`otpauth://` provisioning URI** — the Key Uri Format that Google Authenticator, Authy,
1Password, Microsoft Authenticator and effectively every other authenticator app reads.
This tool takes that link apart and shows you every field it carries: the OTP type, the
issuer, the account name, the base32 shared secret, the HMAC algorithm, the code length,
and the time step (TOTP) or starting counter (HOTP).

It is a *reader*, not a code generator: it never contacts a server, and the secret you
paste stays in your browser.

## What you get back

- **type** — `totp` (time-based) or `hotp` (counter-based).
- **issuer** — the provider name in effect. The URI can carry it twice, as a label prefix
  (`ACME Co:alice@example.com`) and as an `issuer=` parameter; the parameter wins, and a
  disagreement between the two is reported.
- **account** — everything after the first colon in the label, percent-decoded and with the
  spec's optional space after the colon trimmed.
- **secret** — normalized to unpadded upper-case base32 and validated against the RFC 4648
  alphabet, plus its length in characters and in decoded bytes.
- **algorithm / digits / period / counter** — with the Key Uri Format defaults (`SHA1`,
  `6` digits, `30`-second period) filled in when the URI omits them and listed under
  `defaults_applied` so you can see what was assumed rather than stated.
- **extra_parameters** — anything outside the spec (e.g. an `image=` icon URL) kept verbatim.
- **warnings** — non-fatal compatibility problems most apps will silently paper over.

## Worked example

Input:

```
otpauth://totp/ACME%20Co:john.doe@email.com?secret=HXDMVJECJJWSRB3HWIZR4IFUGFTMXBOZ&issuer=ACME%20Co&algorithm=SHA1&digits=6&period=30
```

Output with **Readable lines** selected:

```
type:        totp
issuer:      ACME Co
account:     john.doe@email.com
label:       ACME Co:john.doe@email.com
secret:      HXDMVJECJJWSRB3HWIZR4IFUGFTMXBOZ
secret size: 32 chars, 20 bytes
algorithm:   SHA1
digits:      6
period:      30 seconds
```

Pick **JSON** instead for a machine-readable object (including `defaults_applied` and
`warnings`), or **ASCII table** for something you can paste into a terminal or a wiki.

## Output formats and options

- **JSON** — every field as an object, with `null` for the field that does not apply
  (`counter` on TOTP, `period` on HOTP).
- **Readable lines** — aligned `key: value` lines, with a `Warnings (n):` block appended
  when there is anything to flag.
- **ASCII table** — the same rows in a bordered `Field | Value` table.
- **Mask the secret** — replaces the secret with asterisks while keeping `secret_chars`
  and `secret_bytes` accurate, so you can share the parse in a bug report without leaking
  the credential.
- **Strict mode** — turns the spec's *recommendations* into hard errors: a missing issuer,
  an issuer that differs between the label and the `issuer=` parameter, a digit count other
  than 6 or 8, a duplicate parameter, or any parameter outside the Key Uri Format. Useful
  when you are validating URIs you generate rather than diagnosing one you received.

## Limits and edge cases

- One URI per run. A `otpauth-migration://offline?data=…` export (Google Authenticator's
  "transfer accounts" payload) bundles several accounts in a protobuf blob — this tool
  detects it and tells you to expand the export first.
- The input is capped at 4096 characters; real provisioning URIs are well under 300.
- Line breaks and tabs are stripped before parsing, so a URI wrapped across lines by an
  email client still works (you get a warning saying so).
- A label with more than one colon is split on the **first** one, matching how apps read it,
  and warns you that it was ambiguous.
- Percent-decoding is lenient: a stray `%` that is not followed by two hex digits is kept
  literally instead of failing the whole parse.
- Duplicate query parameters keep the **first** value, as most parsers do.
- `secret` is required. Digits outside 1–10, a zero `period`, an HOTP URI with no `counter`,
  an unknown OTP type, and a non-base32 secret are all hard errors, in every mode.

## FAQ

<details>
<summary>Is it safe to paste a real 2FA secret here?</summary>

The parsing runs entirely in your browser via WebAssembly — nothing is uploaded, and there
is no server to log it. That said, a secret is a live credential: if you only need to show
someone the *shape* of a URI, tick **Mask the secret** so the output carries `********`
plus the true `secret_chars` / `secret_bytes` instead of the real value.

</details>

<details>
<summary>My URI has no algorithm, digits, or period. Is it broken?</summary>

No — those parameters are optional. The Key Uri Format says a reader must assume `SHA1`,
`6` digits, and a `30`-second period when they are absent, which is exactly what this tool
does. Every value that came from a default rather than from your URI is listed under
`defaults_applied`, so you can tell the two apart at a glance.

</details>

<details>
<summary>Why does it warn about the issuer when the URI clearly has one?</summary>

The issuer can appear in two places: as the label prefix before the colon, and as the
`issuer=` query parameter. The spec wants **both**, and wants them identical. If only one
is present you get a compatibility warning, because some apps read the other one. If they
disagree (`GitHub:alice` with `issuer=GitLab`) the mismatch is flagged too — the `issuer=`
parameter is what this tool reports as the effective issuer, but apps genuinely differ on
which one they display.

</details>

<details>
<summary>Why is my secret rejected as "not base32"?</summary>

Base32 (RFC 4648) uses only `A`–`Z` and the digits `2`–`7`. The characters `0`, `1`, `8`
and `9` are not in the alphabet, so a "secret" containing them is either hex, base64, or a
typo — an authenticator app would generate wrong codes from it. Spaces, hyphens, underscores,
`=` padding and lower case are all fine: they are normalized away first, and you get a
warning saying the secret was cleaned up. A length that can't come from whole bytes (like 3
characters) is rejected for the same reason.

</details>

<details>
<summary>What's the difference between the period and the counter?</summary>

`period` belongs to **TOTP**: it is the number of seconds each code is valid for, normally
30. `counter` belongs to **HOTP**: it is the starting value of a counter that advances every
time a code is used. A URI should carry exactly one of them. An HOTP URI without a counter
is an error; a `counter=` on a TOTP URI (or a `period=` on an HOTP one) is reported as an
ignored parameter rather than a failure, because that is how apps treat it.

</details>

<details>
<summary>Can it decode the QR image itself, or a migration export?</summary>

Not here — this tool starts from the URI text. Scan or decode the QR code first with a QR
reader, then paste the `otpauth://…` string it produces. Multi-account
`otpauth-migration://` exports are a different, protobuf-encoded format and need to be
expanded into individual `otpauth://` URIs before this parser can read them.

</details>
