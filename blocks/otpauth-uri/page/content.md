## About this tool

This tool builds a standard **`otpauth://` provisioning URI** — the Key Uri Format
understood by Google Authenticator, Authy, 1Password, Microsoft Authenticator, and
virtually every other authenticator app. Encode the resulting URI as a QR code and scan
it to add a new two-factor (2FA) account, or paste it directly into an app that accepts
setup links.

Everything runs locally in your browser. The secret you enter is never sent to a server.

## What goes into the URI

- **Type** — `totp` (time-based, the usual rolling 2FA codes) or `hotp` (counter-based).
- **Issuer** — the provider or organization name shown in the app (e.g. `GitHub`). Recommended
  so accounts are easy to tell apart.
- **Account name** — the username or email the codes are for. It must not contain a colon.
- **Secret** — the shared secret, base32-encoded (RFC 4648). Spaces and letter case are ignored.
- **Digits** — how many digits each code has (6, 7, or 8; default 6).
- **Period** — the TOTP time step in seconds (default 30; ignored for HOTP).
- **Algorithm** — the HMAC hash: `sha1` (the authenticator-app standard), `sha256`, or `sha512`.
- **Counter** — the HOTP starting counter value (default 0; ignored for TOTP).

## The format

```
otpauth://TYPE/ISSUER:ACCOUNT?secret=SECRET&issuer=ISSUER&algorithm=ALG&digits=N&period=SECONDS
```

The label and issuer are percent-encoded so values with spaces or special characters stay valid.
For HOTP, `period` is replaced by `counter`.
