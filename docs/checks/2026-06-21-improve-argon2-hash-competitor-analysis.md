# argon2-hash — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/argon2-hash` — hash a password with Argon2id (configurable
memory, iterations, parallelism) returning the PHC string, and verify a password
against one. Pure-Rust (`argon2`). Pure-text input → text output: chat + CLI + a
page.

## What competitors do

- **Online bcrypt/argon2 generators** — paste a password, get a hash. **Weakness:
  you paste a password into a third-party page**; many only do bcrypt, or use weak
  defaults.
- **Language libs** (`argon2` in Python/PHP/Node) — correct, but require a runtime
  and code; the right tool for production but not a quick one-off.
- **`argon2` CLI** (from the reference implementation) — local, but a native
  install and a non-obvious flag interface.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (`argon2`, RustCrypto) compiled
   to wasm: chat Service Worker, CLI, and in-browser page. The password never
   leaves the device.
2. **Argon2id, the recommended algorithm**, with a fresh random salt per hash and
   **standard PHC output** (`$argon2id$v=19$m=…,t=…,p=…$salt$hash`) that any
   Argon2 library can verify — portable across stacks.
3. **Tunable + sane defaults.** Memory (KiB), iterations and parallelism are
   configurable; the defaults (19 MiB / 2 / 1) follow current OWASP guidance.
4. **Verify built in.** `mode=verify` checks a password against a PHC string,
   reading the parameters from the string itself — so the tool both produces and
   checks hashes.
5. **Same everywhere.** Identical via chat, CLI, and a `?password=…&mode=…` page.

## Honest scope

- **Argon2id only** (not bcrypt/scrypt/PBKDF2) — the current best-practice choice;
  other algorithms are separate tools.
- It hashes/verifies; it does not store hashes or manage users.
- Very high memory settings will be slower in the browser/SW (Argon2 is
  intentionally memory-hard) — the defaults are a good balance.

## Tests

5 core unit tests: a hash is a `$argon2id$` PHC string carrying the chosen
`m=`/`t=` params; **two hashes of the same password differ** (fresh salt); a
**verify round-trip** (correct password → true, wrong → false); verifying a hash
made with different params (params read from the PHC); and error cases (empty
password, too-low memory, non-PHC verify input). Plus the block drift-guard schema
test. **CLI verified** end-to-end (hash → verify). **Page** verified with
Playwright (hash then verify in-browser). `wafer build` instantiates the chat
block.
