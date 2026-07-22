# otpauth-qr-generator — competitor analysis (2026-07-22)

Scan of real tools that generate QR codes for `otpauth://` authenticator provisioning URIs. Findings are paraphrased; no competitor copy or branding is reused.

## Competitors

### 1. 2FA Fast — 2FA QR Code Generator
Browser tool focused on creating a standard TOTP `otpauth://` URI and rendering it as a scannable QR code. Table stakes: issuer/service, account label, base32 secret, algorithm/digits/period options, and a visible provisioning URI/QR result. UX pattern: simple form, generate button, scan-ready QR.

### 2. Encode64 — OTP Authenticator QR Code Generator
Online form for creating authenticator setup QR codes. Table stakes: issuer, account, secret, TOTP fields, compatibility with common authenticator apps, and an generated QR result. UX pattern: field-based form with copy/scan result.

### 3. Deepnet Security — OTP QR Generator
Security vendor utility for OTP provisioning QR codes. Table stakes: type/issuer/account/secret and advanced OTP fields; notes that some advanced options may be ignored by authenticators. UX pattern: visible fields and generated QR.

## Table stakes → implementation

| Capability | Status in our tool |
|---|---|
| Full `otpauth://` URI passthrough | Built — `uri` encodes a valid TOTP/HOTP provisioning URI verbatim. |
| Build URI from fields | Built — issuer, account, base32 secret, OTP type, algorithm, digits, period, counter. |
| TOTP and HOTP | Built — `otp_type=totp|hotp`; HOTP uses `counter`. |
| Algorithm choices | Built — SHA1/SHA256/SHA512 enum. |
| Digit count | Built — 6..8. |
| TOTP period | Built — 1..600 seconds. |
| QR error correction | Built — L/M/Q/H enum. |
| Output format | Built — SVG default or PNG. |
| QR colors | Built — dark/light hex colors, including short hex. |
| Authenticator import intent | Built — descriptor explains scan/import behavior and local secret handling. |

## Considered but not built

- Secret generation: distinct responsibility already covered by OTP/TOTP secret or token tools; this row renders provisioning QR codes from a provided secret/URI.
- Live current-code generation: covered by TOTP/HOTP code tools; enrollment QR generation is the distinct surface here.
- Logo embedding / rounded modules / branded templates: visual customization beyond the generic utility model and unnecessary for authenticator scanner compatibility.
- Hosted account storage or syncing: out of model; this repo ships stateless local tools.

## Not a duplicate

`otpauth-uri` builds/canonicalizes provisioning URIs but does not render a scannable image. `totp-generator` / `generate-hotp` compute OTP codes from a secret. This tool adds the QR image output needed for app enrollment and is therefore distinct.
