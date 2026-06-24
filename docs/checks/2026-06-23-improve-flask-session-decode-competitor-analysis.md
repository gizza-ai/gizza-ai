# flask-session-decode — competitor analysis (2026-06-23)

Tool: decode a Flask / itsdangerous session cookie into readable JSON **without** the
`SECRET_KEY`. Surfaces: chat skill block, CLI (`gizza tool flask-session-decode`),
standalone page (`/tools/flask-session-decode/`).

## Surfaces verified (Phase 1)

| Surface | Status | Evidence |
| --- | --- | --- |
| core unit tests + drift-guard | PASS | 9 core tests + `schema_json_matches_authored_chat_schema` |
| chat block (`wafer build`) | PASS | block.wasm validates/instantiates, 345.5 KiB |
| CLI | PASS | `gizza tool flask-session-decode cookie=…` → correct JSON |
| page (Playwright) | PASS | `tool-page-flask-session-decode.spec.ts` 3/3 (decode, strip `session=…` fragment, error path) |

## Competitor set

1. **kirsle.net Flask Session Cookie Decoder** (the most-cited free web decoder).
   Shows raw decoded contents + pretty JSON, handles `.`-prefixed zlib payloads,
   decodes without the secret key. **Does NOT decode the timestamp or break out the
   signature.**
2. **noraj/flask-session-cookie-manager** (web + Python) and **Flask-Unsign**
   (Paradoxis, CLI) — decode **and** encode/forge **and** brute-force the secret key.
3. **byceps/flask-cookie-decoder** — minimal Python decoder (payload only).
4. **HackIndex Flask Cookie tool** — decode / verify (with key) / forge.

## Capability diff + gap ranking (fit-to-model)

| Capability | Competitors | This tool | Action |
| --- | --- | --- | --- |
| Decode payload to JSON, no key | all | yes | met |
| Transparent zlib inflate (`.`-prefix) | kirsle, noraj | yes | met |
| Accept full `session=…; Path=/` fragment / quotes | partial | yes (`normalize_cookie`) | **ahead** |
| Decode the signed **timestamp** → Unix + ISO-8601 | flask-unsign (CLI only); **kirsle/byceps: NO** | yes (`timestamp`, `timestamp_iso`) | **ahead of the leading web decoder** |
| Surface the signature segment + `signature_verified=false` flag | partial | yes | **ahead** (explicit "not verified" honesty) |
| Non-JSON / non-UTF-8 payload fallback | varies | yes (string / byte-count note) | met |
| **Verify** the HMAC signature (needs `SECRET_KEY`) | noraj, HackIndex, flask-unsign | NO | **out of model** — by design key-free; the existing `jwt-verify`-style HMAC path would need the user's secret. Not built. |
| **Encode / forge** a cookie (needs key) | noraj, Flask-Unsign | NO | **out of model** — write-side + attack tool; needs the secret key. Not built. |
| **Brute-force** the secret key | Flask-Unsign | NO | **out of model** — needs a wordlist + is an offensive-only feature. Not built. |
| `--legacy` itsdangerous timestamp (pre-0.14) | Flask-Unsign | NO | **edge case** — modern Flask uses the big-endian-int-since-2011 format implemented here; legacy decimal-string timestamps are rare. The decoder never crashes on them (it just reports the raw integer), so no hard failure. Documented as a known limitation. |

## Gaps closed this pass

The tool was authored already covering the in-model superset of the leading free
web decoder (kirsle), so no capability code changes were required after Phase 1.
Concretely, versus kirsle it additionally: decodes the **timestamp** to a Unix time
+ ISO-8601 UTC string, breaks out the **signature** with an explicit
`signature_verified: false`, and normalizes a pasted `session=…; Path=/; HttpOnly`
fragment (name/quotes/attributes stripped). Copy/UX: the page documents the
3-segment format, the 2011 itsdangerous epoch, the signed-not-encrypted security
note, and that the signature is reported but not verified.

## Out-of-model features (intentionally not built)

- **Signature verification / forging / encoding** — all require the application's
  `SECRET_KEY`, which this key-free inspection tool deliberately never takes.
- **Secret-key brute-force** — offensive, needs a wordlist; out of scope.
- **`--legacy` timestamp format** — modern Flask is covered; legacy decimal-string
  timestamps degrade gracefully (raw integer) rather than erroring.

No competitor copy, branding, or trademarks were reproduced.
