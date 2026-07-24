# password-pwned-check — competitor analysis (2026-07-23)

Function: check whether a password appears in known data breaches **without ever sending the
password or its full hash anywhere**, using the Have I Been Pwned (HIBP) Pwned Passwords
k-anonymity range API. All notes below are paraphrased from public docs/tools — no competitor
copy, branding, or trademarks are reproduced.

## Competitors scanned

1. **HIBP Pwned Passwords (official) + API v3 docs** (haveibeenpwned.com/Passwords, /api/v3) —
   the source of truth. Client SHA-1-hashes the password, sends only the first **5 hex chars** of
   the (uppercase) hash to `GET https://api.pwnedpasswords.com/range/{prefix}`; the server returns
   every breached suffix (the remaining **35 hex chars**) sharing that prefix, one per line as
   `SUFFIX:COUNT`. The client matches its own suffix locally and reads the breach count. Optional
   **`Add-Padding: true`** request header pads every response to ~800–1000 entries so a passive
   observer can't infer the bucket from response size; padding rows carry a **count of 0** and must
   be ignored. NTLM prefixes are also supported (separate mode).
2. **1Password Watchtower** — the canonical consumer integration; same protocol (SHA-1 → 5-char
   prefix → local suffix match). Presents a plain "this password appeared in a breach, change it"
   verdict rather than raw hashes. Emphasises "your password never leaves the device".
3. **Bitwarden data-breach report / Firefox Monitor / Chrome compromised-password warning** — all
   consume the same k-anonymity range endpoint; UX is a boolean "exposed / not exposed" plus the
   times-seen count and a recommendation to change reused passwords.
4. **Online one-off checkers (Timbrica, ALLYX ONE, "isitpwned", averybiteydinosaur)** — web forms
   that hash client-side and query the range API; table-stakes copy = show found/not-found, show
   the times-seen count, and reassure that only a 5-char prefix is transmitted.
5. **CLI / library tools (`thiamsantos/pwned` npm, `pwnedpasswords` PyPI, PowerShell
   HaveIBeenPwned, bash one-liners)** — expose a single "password" input, return
   `{ pwned: bool, count: n }`. Some expose a padding flag; all uppercase the hash and compare the
   35-char suffix case-insensitively (HIBP returns uppercase).

## Table-stakes → decision

| Capability | In/out of model | Decision |
|---|---|---|
| SHA-1 hash locally, send only 5-char prefix | in-model (`sha1` crate is wasm-proven) | **build (core)** |
| Local suffix match against `SUFFIX:COUNT` lines | in-model | **build (core)** |
| Return `found` boolean + breach `count` | in-model | **build** |
| Optional `Add-Padding: true` header + ignore count-0 padding rows | in-model | **build (`padding` param)** |
| Uppercase hash + case-insensitive 35-char suffix compare | in-model | **build** |
| Clear network/API error messages (non-200, empty prefix) | in-model | **build** |
| Never transmit/log the raw password or full 40-char hash | privacy requirement | **build (only the prefix is ever sent; output omits the full hash)** |
| Human guidance ("appeared N times — do not use it") | in-model | **build (message field)** |
| **NTLM** prefix mode | in-model (an `nt-hash` block exists) but out of THIS tool's SHA-1 scope | **deferred** — this tool is SHA-1-only per its description; NTLM is a possible future enhancement, listed not built. |
| Password **strength / entropy / crack-time** scoring | in-model but a **different tool** | **out of scope** — already covered by `blocks/password-entropy` / `blocks/weak-password-detector`; this tool answers only "is it in a breach corpus". |
| Bulk / multi-password checking | in-model but scope creep | **deferred** — single-password keeps the descriptor and the k-anonymity privacy story clean. |

## Design

- Descriptor params: `password` (required string), `padding` (optional boolean, default false).
- No page (network block, like `web-fetch`): chat + CLI only. The chat runtime performs the fetch
  via `wafer-run/network`; there is no headless browser surface for a network block.
- Output JSON (flat `ToolResp`): `found` (bool), `count` (u64), `prefix` (the 5 hex chars actually
  transmitted — safe to echo), `padding` (bool, whether padding was requested), `message` (human
  summary). **Deliberately omits the full SHA-1 hash and the suffix** so the response can never be
  used to reconstruct more than what already left the device (the 5-char prefix).

## Non-duplication

No existing block checks a password against a breach corpus. `password-entropy` /
`weak-password-detector` score strength offline against heuristics + a small built-in common-list;
neither queries HIBP nor answers "has this exact password leaked in a real breach". Distinct
capability → viable, not a skiplist.
