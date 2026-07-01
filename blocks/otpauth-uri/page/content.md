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

## FAQ

<details>
<summary>Why is my secret being rejected?</summary>

The secret must be **base32** (RFC 4648): only the letters `A–Z` and digits `2–7`,
with optional `=` padding. Spaces and dashes are stripped and lowercase is fine —
`jbsw y3dp ehpk 3pxp` works — but characters like `0`, `1`, `8`, or `9` mean the
string isn't base32 and the tool errors instead of producing a URI your app can't
import.

</details>

<details>
<summary>Should I change the algorithm, digits, or period?</summary>

Usually not. `sha1`, 6 digits, and a 30-second period are what the service you're
pairing with almost certainly expects, and several authenticator apps silently
ignore non-default values — which would make your codes wrong. Only deviate
(sha256/sha512, 7–8 digits, other periods) when the service's docs explicitly say
so. Digits outside 6–8 are rejected.

</details>

<details>
<summary>What's the difference between TOTP and HOTP?</summary>

**totp** codes roll over on a timer (the `period`, default 30 s) — this is what
nearly every 2FA setup uses. **hotp** codes advance on a counter instead: the URI
carries a starting `counter` value (default 0) and the period is ignored. Pick
HOTP only if the service explicitly issues counter-based tokens.

</details>

<details>
<summary>Why can't the account name or issuer contain a colon?</summary>

The URI's label is `issuer:account`, so a literal colon inside either value would
change where the split happens when the app parses it back. The tool rejects
colons up front; everything else (spaces, @, unicode) is fine because the label
and issuer are percent-encoded.

</details>
