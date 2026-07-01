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

## FAQ

<details>
<summary>Why doesn't the code match what my authenticator app shows?</summary>

Almost certainly because your app is doing **TOTP**, not HOTP — most
authenticator apps derive the counter from the current time, while this tool
uses the exact counter you type. For time-based codes use the
[TOTP generator](/tools/totp-generator/). If you really are comparing HOTP to
HOTP, check that the counter, digits, and algorithm match on both sides.

</details>

<details>
<summary>What format does the secret have to be in?</summary>

Base32 (the RFC 4648 alphabet, `A–Z` and `2–7`), which is how 2FA secrets are
normally handed out. Spaces and lower-case letters are fine — they are stripped
and upper-cased before decoding — and padding `=` signs are not required.
Anything that isn't valid base32 is rejected with an error.

</details>

<details>
<summary>Which digit counts and algorithms are supported?</summary>

Codes can be 6, 7, or 8 digits (6 is the default and what virtually every
service expects), and the HMAC can use SHA-1 (the RFC 4226 standard), SHA-256,
or SHA-512. Both sides of a login must agree on these settings or the codes
won't line up.

</details>

<details>
<summary>Will the same counter always give the same code?</summary>

Yes — HOTP is fully deterministic, so counter 42 with the same secret, digits,
and algorithm produces the same code every time. That's by design: you
increment the counter once per use, and validating servers usually accept a
small look-ahead window in case the client counter runs ahead.

</details>
