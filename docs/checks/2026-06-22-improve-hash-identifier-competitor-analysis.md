# hash-identifier — competitor analysis (2026-06-22)

Tool: `blocks/hash-identifier` — paste a hash string, get the likely algorithm(s).
Pure (structural, no I/O), runs in chat / CLI / standalone page.

## Top competitors surveyed

1. **Toolsana — Hash Identifier** (toolsana.com/tools/hash-identifier) — "100+ hash
   types including MD5, SHA-1/2/3, bcrypt, Argon2, scrypt, PBKDF2", shows **Hashcat
   mode numbers** per match.
2. **BrowserUtils — Hash Identifier** (browserutils.dev) — paste a hash, lists likely
   algorithm; MD5/SHA-1/256/512/bcrypt/argon2; **runs entirely in browser**.
3. **Atbash Cipher — Hash Identifier** (atbashcipher.com) — MD5/SHA/bcrypt/Argon2/CRC32
   + can verify text against common digests.
4. **Pacgie — Hash Identifier** (pacgie.com) — MD5/SHA/bcrypt/Argon2 + "and more".
5. **HashTools / ThisDevTool / hashgenerator.co** — identify-by-format, length +
   prefix heuristics, candidate list.

## Feature diff (us vs. them)

| Capability | Competitors | gizza hash-identifier |
|---|---|---|
| Prefix formats (bcrypt, argon2*, $5$/$6$, $1$, PHPass, apr1, Cisco 8/9, LDAP, MySQL `*`, NetNTLM) | yes | **yes** |
| Bare hex length → family (MD5/NTLM/MD4; SHA-1/224/256/384/512; SHA-3; BLAKE2) | yes | **yes** |
| Base64 digest by decoded length | partial | **yes** |
| Multiple candidates, confidence-ranked | yes | **yes** (high/medium/low) |
| Hashcat mode (`-m`) numbers per match | yes (Toolsana) | **added in this pass** |
| Runs locally / private | yes | **yes** (WASM, no upload) |
| Chat + CLI surfaces | no (web only) | **yes** (3 surfaces) |

## Gaps closed in this pass

- **Hashcat mode numbers.** The strongest differentiator competitors offer is the
  Hashcat `-m` mode per candidate (so a pentester can pipe straight into a crack).
  This is a static lookup, fully in-model — added a `hashcat_mode` field to each
  candidate and surfaced it in the report (e.g. `MD5 [medium] (hashcat -m 0)`).

## Out-of-model / deliberately not built

- **Actual hash cracking / wordlist attacks** — out of scope (and out of model): the
  tool only classifies format, it never recovers the plaintext. Stated explicitly in
  the skill description and page copy.
- **Verifying a candidate by hashing a guessed plaintext** (Atbash's extra feature)
  would require the user to supply the original string and is a different tool
  (a hash *generator/checker*, which gizza already has separately, e.g. `sha256-hash`,
  `bcrypt-hash`). Not merged in to keep this tool single-purpose.
- **"100+ types with John/Hashcat coverage"** marketing claim — we cover the common,
  unambiguously-detectable formats; exotic single-vendor formats are intentionally
  omitted to avoid false confidence on ambiguous bare digests.

## Verification (all surfaces)

- `cargo test --workspace` in `blocks/hash-identifier` — core unit tests + descriptor
  drift-guard pass.
- `wafer build` — chat `block.wasm` validates + instantiates (300 KiB).
- CLI `gizza tool hash-identifier input=…` — bcrypt, MD5 (→ MD5/NTLM/MD4/…), SHA-256
  all classified correctly.
- Playwright `tool-page-hash-identifier.spec.ts` — page identifies bcrypt and lists
  MD5 + NTLM for a 32-hex input (2 passed).

NEVER copies competitor copy/branding; all names are generic algorithm names.
