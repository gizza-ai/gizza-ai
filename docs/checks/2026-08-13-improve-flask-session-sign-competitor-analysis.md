# flask-session-sign — competitor analysis (2026-08-13)

Scan run BEFORE implementation, per `/improve-tool` Phase 2/3. All findings are **paraphrased**;
no competitor copy, branding, or trademarks are reproduced or reused. Competitor names appear only
as factual references to publicly documented behavior.

## Scope

Function under study: *build a valid Flask session cookie from a JSON payload and a `SECRET_KEY`* —
i.e. reproduce `itsdangerous.URLSafeTimedSerializer.dumps()` as Flask's
`SecureCookieSessionInterface` configures it (salt `cookie-session`, `TaggedJSONSerializer`,
SHA-1 digest, `hmac` key derivation).

## Competitors reviewed (top 3 + the reference implementation)

| # | Tool | Kind | Signing support |
|---|------|------|-----------------|
| 1 | Flask-Unsign (Paradoxis) | Python CLI | Yes — `--sign` |
| 2 | HackIndex "Flask cookie" tool | Web tool | Decode / verify / sign tabs |
| 3 | KeyDecryptor "Flask cookie" tool | Web tool | Decode / verify / sign tabs |
| — | `itsdangerous` + `flask.sessions` | Reference library | The ground truth we replicate |

Two more of the top search hits (kirsle.net wizard, picoCTF-solutions decoder) are **decode-only**
and were not counted as signing competitors — there are fewer than 5 real signing competitors, so
this scan reports what is actually real rather than padding the list.

## Table-stakes parameters, defaults, and behaviors observed

| Capability | Competitor behavior | Our decision |
|---|---|---|
| JSON/dict payload input | All 3. Flask-Unsign takes a **Python dict literal** (`{'logged_in': True}`); the web tools take JSON | **In model** — `payload`, strict JSON (documented; Python-literal input is out of scope, see below) |
| Secret key | All 3, plain text | **In model** — `secret` |
| Salt, default `cookie-session` | Both web tools expose it; Flask-Unsign hard-codes it | **In model** — `salt`, default `cookie-session` |
| Hash / digest algorithm | Both web tools expose a selector | **In model** — `digest` enum `sha1` (Flask default) / `sha256` / `sha512` |
| Key derivation method | Both web tools expose a selector | **In model** — `key_derivation` enum `hmac` (Flask default) / `django-concat` (itsdangerous default) / `concat` / `none` |
| Timestamp handling | Web tools expose it; Flask-Unsign always uses "now" | **In model** — `timestamp` (Unix seconds, `0` = now). Explicit timestamps make the output reproducible/testable, which no competitor offers |
| Legacy itsdangerous epoch | Flask-Unsign `--legacy` | **In model** — `legacy_epoch` boolean. Verified against the library: itsdangerous **≥ 1.0** signs the full Unix timestamp; **< 1.0** signed an offset from 2011-01-01 (Unix 1293840000). Changelog 1.0.0: "use the full timestamp rather than an offset, allowing dates before 2011" |
| zlib compression | Automatic in all 3 (inherited from itsdangerous) | **In model, and extended** — `compress` enum `auto` (itsdangerous rule: use zlib only if it saves >1 byte) / `always` / `never`. Forcing the mode is a differentiator; every competitor is auto-only |
| Secret encoding (bytes keys) | KeyDecryptor notes a `b'...'` prefix convention; the others assume UTF-8 | **In model** — `secret_encoding` enum `utf8` / `hex` / `base64`, which is unambiguous where a `b'…'` prefix is not |
| Cookie name / `Set-Cookie` output | Not offered by any of the 3 | **In model** (gap we close) — `cookie_name` + a ready-to-paste `Set-Cookie` header in the output |
| Segment breakdown in the output | HackIndex/KeyDecryptor show it on the **decode** side only | **In model** (gap we close) — the sign output returns `payload_segment` / `timestamp_segment` / `signature_segment`, the serialized payload, and the derived key so a user can cross-check each stage |
| Browser 4096-byte cookie limit | Not surfaced by any competitor | **In model** (gap we close) — `cookie_bytes` plus a warning when the `name=value` pair exceeds the ~4096-byte per-cookie browser cap |

## UX controls observed

- Mode tabs (decode / verify / sign) on both web tools. **Rejected, by design**: gizza ships one
  tool per job and `flask-session-decode` already covers the keyless decode side. Cross-linking in
  the page copy is the right answer, not a mode switch that changes what every field means.
- Advanced options collapsed behind a disclosure. **Not applicable** — the shared page generator
  renders a flat form; the fields are ordered so the two required ones come first.
- A worked sample cookie + decoded output. **Adopted** as `[[example]]` preset chips and a worked
  example in the page copy (our own payload/secret/output, computed by this tool).
- FAQ block (~6 topics on KeyDecryptor: encryption vs signing, the default salt, verification
  failures, compressed cookies, CTF use, CLI comparison). **Adopted as topics**, answered in our
  own words with our own specifics.

## Out-of-model (considered, not built)

- **Secret-key brute force / wordlist cracking** (Flask-Unsign's headline feature). Out of model:
  it is a long-running loop over a large wordlist, and gizza tools are single-shot pure functions.
- **Fetch the cookie from a live URL** (Flask-Unsign `--url`). Out of model for the page (no
  server, CORS) and out of scope for a pure block.
- **Python dict-literal payload input** (`{'logged_in': True}`, Flask-Unsign's input format).
  Considered, rejected: it needs a Python literal parser, and JSON is what every other surface
  (chat, CLI, page query-param) already speaks. Documented on the page: use `true`/`false`/`null`,
  not `True`/`False`/`None`.
- **Flask tagged types** (`datetime`, `bytes`, `tuple`, `uuid`, `Markup`) — not expressible in
  plain JSON input. Partially closed: we DO implement itsdangerous/Flask's `TagDict` escape, so a
  payload whose object has exactly one key named ` t`/` b`/` m`/` u`/` d`/` di` is escaped exactly
  as Flask would. Documented as a limit on the page.
- **`ensure_ascii = False`** payload serialization. Flask's default provider is `ensure_ascii =
  True`, which we implement byte-for-byte (non-ASCII → `\uXXXX`). An app that overrides the JSON
  provider is out of scope; documented as a limit rather than added as an 11th parameter.
- **Verify / round-trip check against an existing cookie.** Out of scope for a signer; the
  reproducible-`timestamp` parameter lets a user re-sign and string-compare instead.

## Follow-up noted for another run (NOT changed here)

`blocks/flask-session-decode/core/src/lib.rs` adds `ITSDANGEROUS_EPOCH` (1293840000) to every
decoded timestamp. Per the itsdangerous 1.0.0 changelog and current `timed.py`
(`get_timestamp()` → `int(time.time())`), that offset applies only to itsdangerous **< 1.0**, so
that tool reports modern cookies' timestamps ~41 years in the future. Out of scope for this build
(it is a different block); recorded here so it can be picked up as its own change.
