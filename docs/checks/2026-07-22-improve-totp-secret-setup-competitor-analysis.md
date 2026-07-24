# totp-secret-setup — competitor analysis (2026-07-22)

Scan of real TOTP setup/secret generator tools. Findings are paraphrased; no competitor copy or branding is reused.

## Competitors

### 1. Encode64 — TOTP Secret Generator
Generates a base32 shared secret and an otpauth provisioning QR/URI for authenticator enrollment. Table stakes: issuer, account, random secret, otpauth URI, QR export, and common TOTP settings.

### 2. Go Tools — TOTP Code/Setup utilities
Offers TOTP tooling around secrets and authenticator-compatible enrollment. Table stakes: base32 secret, issuer/account labeling, otpauth URI, digit count, period, and algorithm options.

### 3. 2FA setup / QR generator tools
Browser tools commonly combine secret entry/generation with QR rendering. Table stakes: standard `otpauth://totp/` URI shape, SHA1 default, 6 digits, 30-second period, and compatibility guidance for authenticator apps.

## Table stakes → implementation

| Capability | Status in our tool |
|---|---|
| Generate random base32 TOTP secret | Built — CSPRNG bytes encoded as RFC4648 base32 without padding. |
| Configurable secret strength | Built — `secret_bytes` 10..64; default 20 bytes / 160 bits. |
| Produce otpauth enrollment URI | Built — uses shared `otpauth-uri` core to validate and percent-encode. |
| Issuer/account labeling | Built — issuer optional/recommended, account required. |
| Algorithm choices | Built — sha1/sha256/sha512 enum. |
| Digit count | Built — 6..8. |
| TOTP period | Built — period seconds, default 30. |
| Local-only secret generation copy | Built — descriptor/page copy states local generation and secret handling. |
| QR code output | Covered by sibling `otpauth-qr-generator`; this tool intentionally returns secret + URI so the setup can be audited/copied. |

## Considered but not built

- QR rendering in this block: built as the distinct `otpauth-qr-generator` tool; duplicating it here would blur the surfaces and duplicate code.
- Current TOTP code generation: covered by `totp-generator`; this row creates setup material, not live codes.
- Account storage, backup, or sync: out of model; this repo ships stateless local utilities.

## Not a duplicate

`otpauth-uri` assembles a URI from a provided secret, but it does not generate a new random secret. `totp-generator` computes current codes from an existing secret. `otpauth-qr-generator` renders a QR from an existing URI/secret. This tool is the setup-material generator: fresh secret plus enrollment URI.
