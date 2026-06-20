# password-entropy — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/password-entropy` — estimate a password's strength in bits
from its character set + length, with a crack-time estimate and weakness flags.
Chat + CLI + page (pure-string, no deps).

## What competitors do

- **Online "password strength" meters** (password-checker sites, "how secure is
  my password" pages) — type a password, get a score/time. The big concern:
  several **transmit the password to a server** (the worst thing to do with a
  password), and many give an opaque score with no explanation.
- **zxcvbn (Dropbox)** — excellent, explanation-rich client-side estimator;
  heavier (large dictionaries). This tool is a lightweight in-Rust estimator.

## How this tool competes / improves

1. **Runs locally — the password never leaves the device.** Pure-Rust compiled
   to wasm: page in-browser, CLI headless, chat Service Worker. The description and
   page state this explicitly (the #1 trust issue for such tools).
2. **Transparent entropy model.** Reports the **bits**, the **alphabet size**, the
   **length**, and how they combine (length × log2(pool)) — not a black-box 0–100
   score.
3. **Actionable weakness flags.** Too short, single character type, (contains a)
   common password, single repeated character, sequential run (1234/abcd) — so the
   user knows *why* it's weak and how to fix it.
4. **Crack-time estimate** at a realistic fast offline rate (~10^10 guesses/s,
   average case), formatted human-readably and capped for very strong passwords.
5. **Three surfaces + deep-links** (handy for quick checks or scripting policy).

## Honest scope

- The entropy model assumes uniform-independent characters — it **overestimates**
  for human-chosen passwords with structure; the pattern/common-password flags
  partly compensate, but it is a heuristic, not zxcvbn-grade. The page says so.
- Common-password list is a small built-in set (top ~30), not a full leaked-list
  check.

## Tests

7 core unit tests: charset/bits math (`abc` → pool 26, ≈14.1 bits, Very weak),
mixed-class pool (95), common-password flag, repeated-char + single-type flags,
sequential-run detection (`abcd`, `1234`, and a negative `a1b2c3d4`), a strong
password (≥90 bits, Strong/Very strong, no warnings, non-empty crack time), and
empty-input error. Plus the block drift-guard schema test. CLI + Playwright
(common password via fill; strong password via deep-link) verified — see commit.
