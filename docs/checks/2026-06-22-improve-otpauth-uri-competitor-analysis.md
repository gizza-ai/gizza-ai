# otpauth-uri — competitor analysis (2026-06-22)

## What this tool does
Builds a standard `otpauth://` provisioning URI (the Key Uri Format used by Google
Authenticator, Authy, 1Password, Microsoft Authenticator, etc.) from an issuer, account
name, and base32 secret. Supports `totp` (default) and `hotp`, with configurable digits
(6–8), period, algorithm (sha1/sha256/sha512), and HOTP counter. Output is the
`otpauth://` URI string. Pure-Rust, runs on all three surfaces (chat / CLI / page); the
secret never leaves the device.

## Surfaces verified (Phase 1)
- **Chat block**: `wafer build` OK (gizza-ai/otpauth-uri v0.1.0, 310 KiB).
- **CLI**: `gizza tool otpauth-uri …` — totp, hotp (counter, sha256, 8 digits), and the
  bad-secret error path all produce correct output / exit 1.
- **Page**: `/tools/otpauth-uri/` — 2 Playwright tests pass (totp build, hotp with counter).
  `type` and `algorithm` render as `<select>` (sourced from the manifest enum schema).

## Top competitors surveyed
1. **it-tools.tech – OTP code generator** — generates live TOTP codes *and* shows the
   `otpauth://` URI plus a QR code; secret/issuer/account inputs.
2. **stefansundin/totp (GitHub Pages)** — builds the URI + QR from issuer/account/secret,
   with type/digits/period/algorithm/counter knobs.
3. **freeotp / authenticator setup pages** — render a scannable QR for a given secret.
4. **dan.hersam.com OTP tool** — URI + QR, live code preview.
5. **2fa.run / various "QR for authenticator" generators** — secret → QR + URI.

## Gap analysis (fit-to-model)
| Capability | Competitors | This tool | Decision |
|---|---|---|---|
| `otpauth://` URI from issuer/account/secret | yes | **yes** | in model — covered |
| totp + hotp type toggle | some | **yes** | covered |
| digits / period / algorithm / counter knobs | some | **yes** | covered |
| Correct RFC label encoding (`Issuer:Account`, percent-encoded, `issuer=` param) | mixed | **yes** | covered; many tools omit the `issuer=` query param — we include it (recommended by the spec) |
| base32 validation + normalization (strip spaces/dashes, uppercase) | rare | **yes** | covered — an edge most tools skip |
| Live TOTP code preview | some | no | **out of model** — that is the existing `totp-generator` tool's job; not duplicated here |
| **QR-code rendering** | most | no | **deferred** — a scannable QR is image-bytes output, which does not fit the page's text-output recompute model (image-bytes tools are chat+CLI-only per the build findings). The URI is emitted as text so the user can paste it into any QR generator. Could be added later as a separate chat/CLI QR field, but kept out to keep the page surface clean and the output copy-pasteable. |

## Improvements applied
- Included the `issuer=` query parameter in addition to the `Issuer:Account` label
  (recommended by the Key Uri Format spec; improves app compatibility) — several
  competitor tools omit it.
- Strict base32 validation with a clear error message, plus whitespace/dash tolerance and
  case-insensitive secret input.
- Guard rails: reject colons in issuer/account (they break label parsing), enforce digits
  6–8 and period ≥ 1, omit `period` for HOTP and `counter` for TOTP.
- enum params (`type`, `algorithm`) render as dropdowns on the page.

## No trademarks / copy copied
Only the public, standardized URI format and parameter names are used. No competitor copy,
branding, or trademarks were reproduced.
