# fernet-token — competitor analysis (2026-06-22)

New tool built this run, then reviewed against the live Fernet-tool landscape and
upgraded to close every in-model gap. Source for the comparison: hands-on review of
the common "Fernet online" tools plus the canonical Python `cryptography` Fernet
reference implementation. **No competitor copy, branding, or assets were reused** —
all copy and design here is original; competitors were studied for features/UX only.

## Our tool (as shipped)

- **Three surfaces**, all verified:
  - Chat/LLM skill (`gizza-ai/fernet-token`) — schema single-sourced from `descriptor()`.
  - CLI: `gizza tool fernet-token text=… [key=…] [mode=encrypt|decrypt] [ttl=N]`.
  - Page `/tools/fernet-token/` (pure wasm) + query-param deep links.
- **Encrypt:** text → url-safe Fernet token; blank key auto-generates a fresh 32-byte
  key and returns it.
- **Decrypt:** verifies HMAC-SHA256 (constant-time) then AES-128-CBC decrypts; reports
  the embedded creation timestamp as ISO-8601 UTC.
- **TTL:** optional max token age in seconds on decrypt; rejects expired or
  future-dated tokens. `0` = no check.
- **Spec-exact + interoperable:** passes the published Fernet spec test vector, and
  round-trips bidirectionally with Python's `cryptography.Fernet` (verified at the CLI:
  our token decrypts in Python and a Python token decrypts in our tool).
- **Local-only:** AES/HMAC/RNG all run in-browser via WebAssembly; nothing is uploaded.

## Competitor landscape (paraphrased profiles)

Typical "Fernet online" / symmetric-token tools fall into a few buckets:

1. **General crypto-utility sites** (devglan-style, 8gwifi-style, asecuritysite-style).
   - Offer Fernet alongside many other primitives. Usually do generate-key + encrypt +
     decrypt. Often **server-side** (data posted to their backend) — a privacy gap.
   - Options are minimal; TTL/expiry is frequently absent or not surfaced.
   - UX: plain forms, sometimes a "generate key" button, copy buttons, ad-supported.

2. **Encoder/converter sites** (dencode-style). Strong on encodings; Fernet support,
   where present, is encrypt/decrypt with an auto-generated or pasted key, browser-local,
   but rarely exposes the **timestamp** or **TTL** semantics.

3. **Language-doc playgrounds / gists.** Show the Python `cryptography` Fernet API; not
   really interactive tools — they educate on key format and TTL but you run code yourself.

4. **CLI / library docs** (Python `cryptography`, ports in Go/JS/Ruby). The reference for
   correctness: 32-byte url-safe-base64 key, `gAAAA…` token prefix, `decrypt(token, ttl)`.

## Gap analysis (fit-to-model filter applied)

In-model gaps we **closed** in this build:

- **Key auto-generation** with the key echoed back so users can save it (many tools make
  you generate a key as a separate step). — done (blank key on encrypt).
- **TTL / expiry enforcement on decrypt**, including future-timestamp rejection — a
  feature several competitors omit or bury. — done.
- **Surfacing the embedded creation timestamp** (`created_at` / "Created:" line) so the
  token's age is visible — most tools hide it. — done.
- **Constant-time HMAC verification** and clean, specific error messages (wrong key vs
  tampered vs expired vs bad base64) rather than a generic failure. — done.
- **Reference interoperability** explicitly verified against Python `cryptography` — a
  trust signal competitors rarely demonstrate. — done + documented in page copy/FAQ.
- **Token inspector / structure breakdown** (8gwifi and asecuritysite offer this, often
  on a separate page): an `inspect` mode that decodes a token's version byte, creation
  time, IV (hex), ciphertext size, and HMAC **without the key** — and validates the HMAC
  if a key is given — consolidated into the same tool. — done (`mode=inspect`, all three
  surfaces, unit + CLI + Playwright tested).
- **Privacy posture:** fully browser-local, no upload, works offline — closes the main
  gap vs server-side crypto-utility sites. — inherent to gizza's model.
- **Deep-linkable page** via query params (`?text=…&key=…&mode=decrypt`) for shareable
  decrypt links. — done + Playwright-tested.

Considered, **not built** (out-of-model or out-of-scope for a focused tool):

- **Key rotation / MultiFernet** (try a list of keys on decrypt). Real Fernet feature;
  deferred to keep the single-key UX simple — noted as a future enhancement, not a server
  dependency, so it could be added later.
- **PBKDF2/Scrypt password-derived keys.** In-model (the `encrypt-file` tool already
  derives keys via PBKDF2), and several competitors offer it; deferred to a follow-up to
  keep this tool's key model unambiguous (a Fernet key is exactly 32 base64 bytes).
- **Batch encrypt/decrypt of multiple lines/tokens** — in-model; deferred (single
  token/text per run keeps the I/O simple). A clean future enhancement.
- **Server-side batch / API endpoints** — out of model (no backend, no accounts).
- **Non-text payloads / file encryption** — the existing `encrypt-file` tool covers file
  encryption; keeping this tool text/token-focused avoids overlap.

## Tests run (all green)

- Core unit tests: 15 (spec vector encrypt + decrypt, roundtrip, empty plaintext, TTL
  within/expired/future, wrong key, tampered token, bad key length, invalid base64, wrong
  version, inspect spec vector, inspect HMAC valid/invalid with key).
- Block tests: 5 (chat-schema drift guard, encrypt→decrypt roundtrip, decrypt-without-key
  error, inspect mode with/without key, ISO-8601 formatting incl. epoch + spec timestamp).
- `wafer build` OK (chat block instantiates — getrandom + SystemTime + AES/HMAC all run in
  wafer). `wasm-pack` web build OK. Generator renders the page.
- CLI: encrypt (auto-key + reused key + empty text), decrypt, wrong-key error, TTL
  expiry, inspect (with/without key); bidirectional Python `cryptography.Fernet` interop.
- Playwright (4): encrypt→decrypt roundtrip, wrong-key error, inspect structure
  (with/without key), query-param deep-link decrypt of the spec vector.

## Limitations / honesty notes

- Single-key only (no MultiFernet rotation yet — listed above).
- The page shows the token + key as text (copy via the page's standard copy affordance);
  there is no separate per-field copy button beyond the page chrome's.
