# Competitor analysis — otpauth-uri-parser (2026-08-14)

Tool: `otpauth-uri-parser` — parses a single `otpauth://` provisioning URI into the fields a 2FA authenticator app reads.

Scan performed 2026-08-14. Sources skimmed: Google Authenticator Key Uri Format documentation, Yubico OATH URI documentation, and practical otpauth QR-code explainer/parser writeups. No competitor copy or branding is reproduced in the tool.

## Table stakes

| Capability / UX pattern | Decision |
| --- | --- |
| Accept a pasted `otpauth://totp/...` or `otpauth://hotp/...` URI, including percent-encoded labels and query strings. | In model. `uri` is the required textarea input. |
| Split label into issuer prefix and account, and parse issuer query parameter. | In model. Query issuer wins, label/query mismatch is reported. |
| Required `secret`, normalized base32 validation, and safe secret masking for support tickets. | In model. Secret is normalized/validated; `mask_secret` can hide it while keeping length fields. |
| TOTP defaults: SHA1, 6 digits, 30-second period. HOTP requires counter. | In model. Defaults are applied and listed; HOTP without counter errors. |
| Algorithm/digits/period/counter validation. | In model. SHA1/SHA256/SHA512, digit warnings, period/counter type-specific handling. |
| Human and machine-readable outputs. | In model. `json`, `text`, and `table` output formats with example chips. |
| Strict validation mode for compatibility checks. | In model. `strict` converts issuer mismatch, missing issuer, unknown params, duplicate params, and non-standard digits into errors. |
| QR image decoding. | Out of model for this pure parser. Image QR decoding belongs in a separate image/QR decoding tool; this block intentionally parses the URI text after extraction. |
| Generating a new URI or QR code. | Out of model here and already covered by the existing `otpauth-uri` builder/generator block. |
| Decoding Google Authenticator migration exports. | Out of model here and already covered by `otpauth-migration-decoder`; this parser points migration payloads to that tool. |

## Built checks

- JSON/text/table outputs.
- TOTP and HOTP paths.
- Percent decoding, label issuer/account split, query issuer handling, issuer mismatch warnings.
- Base32 secret normalization and validation, masking option, strict mode.
- Defaults for omitted algorithm/digits/period.
