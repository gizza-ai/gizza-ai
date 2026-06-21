# bcrypt-hash — competitor analysis (2026-06-21)

## Surfaces verified
- **Chat / LLM API**: `wafer build` validates `target/block.wasm` (bcrypt instantiates in
  wasm32-wasip1). Schema single-sourced from `descriptor()`; drift-guard unit test passes.
- **CLI**: `gizza tool bcrypt-hash password='correct horse' cost=6` → `{"hash":"$2b$06$…"}`;
  `mode=verify hash='$2b$10$…'` → `{"match":true}` / `{"match":false}`.
- **Page**: Playwright `tool-page-bcrypt-hash.spec.ts` — hash path (asserts `$2b$06$` + 60-char
  MCF) and verify path (known external `$2b$10$` hash → "match"). Both pass.

## Competitors surveyed (top 5)
1. **bcrypt-generator.com** — hash + decrypt/verify (check string against hash); rounds selector (4–31).
2. **bcrypt.online** — hash with cost slider; separate "verify" tab against a paste-in hash.
3. **8gwifi.org bcrypt** — hash + verify; exposes cost.
4. **devglan bcrypt** — encrypt (hash) + match (verify); cost dropdown.
5. **browserling / akto bcrypt tools** — single-shot hash, fixed-ish rounds.

## Capability diff (all in-model items closed)
| Capability | Competitors | bcrypt-hash |
| --- | --- | --- |
| Generate `$2b$` hash | yes | yes (fresh random salt) |
| Configurable cost / work factor | yes (4–31) | yes, `cost` 4–31, default 12 (sensible 2026 default) |
| Verify password vs existing hash | yes | yes (`mode=verify`) |
| Accept `$2a$/$2x$/$2y$` legacy variants on verify | partial | yes (algorithm-tagged; test covers `$2y$`) |
| 72-byte truncation handling | silently truncate (data-loss footgun) | **explicit error** instead of silent truncation — differentiator |
| Runs locally, nothing uploaded | server-side on most | **fully client-side wasm** (page + chat + CLI) |
| 60-char MCF output, copyable | yes | yes |

## Out-of-model (not built — would need infra/models or out-of-scope inputs)
- Bulk / file-list hashing (page input is a single field) — CLI scripting covers this.
- Hash "cracking" / dictionary attack (intentionally out of scope; this is a hashing tool).

## Notes
- No competitor copy, branding, or trademarks were used.
- Default cost 12 follows current OWASP guidance for bcrypt; each +1 doubles work, surfaced in copy.
- The 72-byte hard error (vs silent truncation) is the main UX/correctness edge over the field.
