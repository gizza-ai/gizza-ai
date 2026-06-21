# jwt-sign — competitor analysis (2026-06-21)

**Tool:** `gizza-ai/jwt-sign` — build and sign a JSON Web Token (JWS compact
serialization) from a JSON payload (+ optional header) using HS256/384/512,
RS256/384/512, or ES256/384, returning the compact `header.payload.signature`
string. Runs locally (Rust → wasm) across all three surfaces: chat skill, CLI,
and the in-browser page.

## Competitors

1. **jwt.io (Auth0/Okta) — Debugger.** The de-facto reference. Decode + verify +
   sign in one panel; algorithm dropdown HS/RS/ES/PS/EdDSA; live header/payload
   editors. Encoding happens client-side. Strengths: ubiquity, EdDSA + PS
   support, public-key verification UI. It is a debugger first; no CLI, no
   scriptable API, no chat/agent surface.

2. **jwt.dev / token.dev.** Lightweight client-side encode/decode playgrounds.
   Good UX, claim helpers (exp/iat presets). Browser-only; no CLI or programmatic
   surface; ad-supported.

3. **jsonwebtoken (npm, node).** The library most apps actually sign with.
   Full-featured (all algs incl. PS/EdDSA, `expiresIn`/`notBefore` sugar). It is a
   dependency, not a tool — needs a Node project and code to produce one token.

4. **jwt-cli (`mike-engel/jwt-cli`, Rust).** A real command-line JWT
   encoder/decoder. Closest analog to gizza's CLI surface. Strong for encode +
   inspect; HS/RS/ES/PS/EdDSA via key files. No browser page, no chat/agent
   integration.

5. **PyJWT / `jwt` CLI (Python).** Reference Python implementation; widely used in
   backends. Library + thin CLI; same "needs an environment + code" friction as
   jsonwebtoken.

## How gizza differs

- **Three surfaces, one implementation.** The same pure-Rust core powers a chat
  skill (an LLM/agent can mint a token mid-conversation), a CLI (`gizza tool
  jwt-sign --json '{...}'`), and a zero-backend browser page. Competitors pick
  one lane (web debugger *or* npm lib *or* CLI).
- **Local-only by construction.** The page runs entirely in WebAssembly — the
  secret, private key, and claims never touch a server. jwt.io encodes
  client-side too, but pasting a production private key into a hosted debugger is
  a recurring security worry; gizza's CLI/chat keep it fully off-network.
- **No project setup.** Unlike jsonwebtoken/PyJWT you don't scaffold a Node/Python
  project to get one signed token.
- **Sensible header defaults.** `alg` is always set from the chosen algorithm
  (you can't accidentally sign with a mismatched header), and `typ` defaults to
  `JWT` while still letting you add `kid` and other JOSE fields.

## Verification

- Core unit tests (`cargo test`): HS256 structure, HS256 byte-exact HMAC match,
  header `alg` override + `typ` default, deterministic HMAC, RS256 verifies with
  the matching RSA public key, ES256 raw `r‖s` (64-byte) signature verifies with
  the P-256 public key, plus error paths (non-JSON, non-object payload, empty
  secret, bad key, bad algorithm).
- Drift-guard test: descriptor schema == authored chat schema == manifest.json.
- `wafer build`: compiles to wasm32-wasip1 **and instantiates/validates** the chat
  block.
- CLI: `gizza tool jwt-sign --json '{"payload":...,"secret":...}'` produces a
  valid 3-part token.
- Page: Playwright (`tool-page-jwt-sign.spec.ts`) checks a 3-part HS256 token,
  algorithm reflected in the header, and a clear error on a non-object payload.

## Honest scope

- **Algorithms supported:** HS256/384/512, RS256/384/512, ES256/384.
- **Not supported (out of current model):**
  - **PS256/384/512 (RSA-PSS):** intentionally omitted from JWT signing here to
    keep RSA output deterministic; could be added (the `rsa` PSS path already
    works in the standalone `rsa-sign` tool).
  - **ES512 (P-521) and EdDSA (Ed25519):** P-521's ECDSA signer in this stack is
    randomized-only (pulls `getrandom`, no RFC-6979 path) and EdDSA isn't wired
    into this core yet; both are deferred to keep the build wasm-safe and
    deterministic. (`ed25519`/`p521` exist elsewhere in the repo as key-gen.)
  - **Claim sugar** (`expiresIn`/`notBefore` relative helpers): the tool signs the
    payload you give it verbatim — set `exp`/`nbf`/`iat` as absolute epoch seconds
    yourself.
  - **Decoding/verification:** this tool only *signs*; decoding/verifying is a
    separate concern.
- Competitor copy, branding, and trademarks were **not** copied.
