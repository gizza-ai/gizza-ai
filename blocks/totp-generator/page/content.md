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
