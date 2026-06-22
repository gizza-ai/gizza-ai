## About this tool

**HOTP generator** turns a **base32 secret** and a **counter value** into a
counter-based one-time code, following **RFC 4226 (HOTP)** — the HMAC-based
one-time password standard used for event-based two-factor authentication and
hardware tokens.

- Paste the **base32 secret** (spaces and case don't matter).
- Enter the **counter** — the event counter that increments by one for each code.
- Optionally change the **digits** (6–8) or **algorithm** (SHA-1 is the standard).
- You get the code for that exact counter value.

### HOTP vs TOTP

**HOTP** is **counter-based**: the moving factor is a counter you increment on
every use, so the same counter always produces the same code. **TOTP** (what
most authenticator apps show) derives that counter from the current time. If you
want time-based 2FA codes instead, use the [TOTP generator](/tools/totp-generator/).

### Privacy

Everything runs **in your browser** via WebAssembly — your secret is **never
uploaded** to a server. You can also run it from the [gizza CLI](/) or inside a
gizza chat. (Treat your HOTP secret like a password — anyone with it can generate
your codes.)

### How it works

It implements **RFC 4226**: an **HMAC** of the 8-byte big-endian counter, keyed
by your secret, then dynamic truncation to a numeric code. This is the open
standard, so the codes match any RFC 4226 implementation.
