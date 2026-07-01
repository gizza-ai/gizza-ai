## About this tool

**TOTP generator** turns a **TOTP secret** (the base32 string behind a QR code
when you set up two-factor authentication) into the current **6-digit code** —
the same code an authenticator app like Google Authenticator or Authy would show.

- Paste the **base32 secret** (spaces and case don't matter).
- Optionally change the **digits** (6–8) or **algorithm** (SHA-1 is the standard).
- You get the current code and how many seconds it stays valid.

### Privacy

Everything runs **in your browser** via WebAssembly — your secret is **never
uploaded** to a server. You can also run it from the [gizza CLI](/) or inside a
gizza chat. (Treat your TOTP secret like a password — anyone with it can generate
your codes.)

### How it works

It implements **RFC 6238 (TOTP)**: an **HMAC** of the current 30-second time-step
counter, keyed by your secret, truncated to a numeric code. This is the open
standard every authenticator app uses, so the codes match.

## FAQ

<details>
<summary>The code doesn't match my authenticator app — what's wrong?</summary>

First check your device clock: TOTP hashes the current time, so a clock that's
off by 30+ seconds produces a different code. Then confirm the parameters
match the service — nearly all use 6 digits / 30-second period / SHA-1, but a
service that chose SHA-256, 8 digits, or a 60-second step needs the same
settings here.

</details>

<details>
<summary>What exactly do I paste as the secret?</summary>

The base32 string from your 2FA setup — the `secret=` value inside the
`otpauth://` QR-code URL, or the "manual entry" key the service shows.
Spaces and lowercase are fine and `=` padding isn't needed, but it must be
base32 (letters A–Z, digits 2–7), not hex.

</details>

<details>
<summary>Can I compute the code for a specific moment in time?</summary>

Yes — supply a Unix **timestamp** and the code is computed for that time-step
instead of now (omit it, or pass 0, for the current time). That's handy for
debugging server-side validation or checking RFC 6238 test vectors.

</details>

<details>
<summary>Isn't SHA-1 insecure? Why is it the default?</summary>

TOTP uses SHA-1 inside HMAC, which is not affected by the collision attacks
that broke plain SHA-1 signatures. HMAC-SHA1 is the RFC 6238 default that
virtually every site and authenticator implements — pick SHA-256/SHA-512 only
if the service explicitly uses them.

</details>
