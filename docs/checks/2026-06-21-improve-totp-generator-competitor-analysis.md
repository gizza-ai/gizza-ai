# totp-generator — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/totp-generator` — generate time-based one-time (2FA) codes from
a TOTP secret (RFC 6238). Pure-Rust (`hmac` + `sha1`/`sha2` + `base32`). Text input
→ text output: chat + CLI + a page.

## What competitors do

- **Authenticator apps** (Google Authenticator, Authy, 1Password) — the normal
  way; secure and convenient, but tied to a device and not scriptable / not usable
  from a terminal or chat.
- **Online "TOTP generator" sites** — paste a secret, get a code. **Weakness: you
  paste a 2FA secret into a third-party web page** — a serious exposure if that
  page is untrustworthy or logs input.
- **`oathtool`** — local + scriptable and excellent, but a native install and CLI
  flags to learn.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust compiled to wasm: the chat
   Service Worker, the CLI, and the in-browser page all compute the code on-device.
   The secret never touches a network — the one property a 2FA-secret tool must
   have.
2. **Standards-correct (verified against RFC 6238).** Implements HOTP/TOTP exactly;
   the core is tested against the official RFC 6238 SHA-1 test vectors, so codes
   match what authenticator apps produce.
3. **Flexible.** Configurable digits (6–8), period, and algorithm (SHA-1 default,
   plus SHA-256/SHA-512 for non-default issuers). Accepts secrets with spaces and
   any case.
4. **Time handled per surface.** The chat block and CLI use the system clock; the
   browser page uses `Date.now()` (wasm32 has no std clock) — and a `timestamp`
   parameter lets you compute a code for any instant (useful for testing / clock
   skew).
5. **Shows validity.** Returns how many seconds the current code stays valid, so
   you know whether to wait for the next window.

## Honest scope

- **Generation, not storage/scanning.** It doesn't store secrets or scan QR codes;
  paste the base32 secret. Treat the secret like a password.
- **TOTP/HOTP numeric codes** — not push-based or FIDO/WebAuthn 2FA.

## Tests

5 core unit tests: the **official RFC 6238 SHA-1 test vectors** (t=59 → 94287082,
plus the 1111111109 / 1111111111 vectors); 6-digit truncation; `seconds_remaining`
math; secrets with spaces and lowercase decode correctly; and error cases (empty /
non-base32 secret, bad digits, bad period, bad algorithm). Plus the block
drift-guard schema test. **CLI verified** end-to-end (a known secret + fixed
timestamp → the RFC code). **Page** verified with Playwright (secret → a 6-digit
code with validity). `wafer build` instantiates the chat block.
