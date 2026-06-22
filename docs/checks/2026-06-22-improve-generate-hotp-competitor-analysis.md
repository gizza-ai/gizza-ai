# generate-hotp — competitor analysis (2026-06-22)

Counter-based one-time password generator (HOTP, RFC 4226) from a base32 secret
and an explicit counter. Pure-Rust (`hmac` + `sha1`/`sha2` + `base32`),
deterministic, runs on all three surfaces (chat / CLI / page). Sibling of the
existing time-based `totp-generator`.

## Surfaces verified

- **Chat block**: `wafer build` validates `target/block.wasm` (339.6 KiB). Schema
  drift-guard unit test passes.
- **CLI**: `gizza tool generate-hotp secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ counter=0`
  → `{"code":"755224","counter":0}` (RFC 4226 Appendix D vector). `digits`/
  `algorithm` overrides and bad-secret error path verified.
- **Page** `/tools/generate-hotp/`: Playwright (2 tests) — RFC counter-0 code +
  bad-secret error. secret/counter/digits are fields, algorithm is a `<select>`.

## Correctness

Validated against the full **RFC 4226 Appendix D** test-vector table (secret
"12345678901234567890", counters 0–9, SHA-1, 6 digits → 755224, 287082, …,
520489) plus an 8-digit and SHA-256 cross-check computed independently in Python.

## Competitor scan (top 5)

| Tool | Counter input | Base32 secret | SHA-1/256/512 | Digits config | In-browser/private |
|------|---------------|---------------|---------------|---------------|--------------------|
| VariedTools OTP generator | yes (+ next-N preview) | yes | sha1 (mainly) | partial | yes |
| MicroApp OTP generator | yes | yes | yes | partial | yes |
| Go Tools TOTP/HOTP | yes | yes | yes | yes | yes |
| IO Tools TOTP/HOTP | secret-gen focused | yes (gen + QR) | n/a | n/a | yes |
| DevToys Web Pro OTP | yes | yes | yes | yes | yes (desktop/web app) |

## Gap analysis (fit-to-model)

Covered (in-model, shipped):
- Base32 secret with whitespace/case tolerance.
- Explicit counter (non-negative integer), the defining HOTP input.
- Configurable digits (6–8) and algorithm (SHA-1 / SHA-256 / SHA-512).
- Client-side / privacy-preserving compute (wasm), matching every competitor's
  "secret never leaves your device" claim.
- Cross-links to the time-based `totp-generator` on the page (HOTP-vs-TOTP copy).

Deliberately out of scope for this single-purpose tool:
- **Next-N-codes preview / validation window**: some tools list the next 10
  codes for a counter or validate a submitted code against a look-ahead window.
  The gizza tool model is single-input → single-output; a caller increments the
  counter themselves. Not built (would change the I/O contract); noted, not copied.
- **Secret generation + QR (`otpauth://`) provisioning**: IO Tools' focus. This
  is a separate concern (secret/QR generation) already partially served by other
  gizza blocks; HOTP generation stays decoupled.

No competitor copy, branding, or trademarks were reproduced.

## Sources

- https://www.variedtools.com/generate-validate-otp
- https://microapp.io/otp-generator/
- https://go-tools.org/tools/totp-generator
- https://iotools.cloud/tool/totp-hotp-generator/
- https://devtoys.pro/generators/otp
- https://datatracker.ietf.org/doc/html/rfc4226
