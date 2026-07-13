# weak-password-detector — competitor analysis (2026-07-13)

Scope: tools that answer "is this password weak / common / breached?". All
observations are paraphrased from public product behaviour; no competitor copy,
branding, or trademarks are reproduced.

## Competitors reviewed

1. **Pwned Passwords (k-anonymity range API)** — checks a password's SHA-1 prefix
   against a huge corpus of breached hashes and returns how many times it has been
   seen. Online lookup; privacy via k-anonymity (only a hash prefix leaves the
   client).
2. **A major password-manager vendor's "how secure is my password" page** — shows
   an estimated time-to-crack and a strength verdict, computed in the browser.
3. **A consumer security-suite password checker** — strength meter plus generic
   composition tips (length, mixed case, symbols, avoid dictionary words).
4. **A well-known "password strength meter" library demo (zxcvbn-style)** — entropy
   / guessability score, pattern detection (dictionary words, sequences, repeats,
   dates, l33t), and a crack-time estimate.
5. **A generic "common password list" checker** — matches the input against a
   fixed top-N worst-passwords list and says whether it appears, sometimes with a
   rank.

## Table-stakes → where each lands in our model

| Table-stake capability | Decision |
| --- | --- |
| Match against a common/worst-passwords list | **In model** — core `detect()` matches a bundled ranked list. |
| Report a rank / "how common" | **In model** — `rank` (1-based) + `severity` band in the result. |
| Catch case-only variations (`PASSWORD` = `password`) | **In model** — `case_sensitive` param (default false). |
| Catch leetspeak / substitution variants (`P@ssw0rd`) | **In model** — `normalize_leet` param (default true). |
| Run privately / offline (no upload) | **In model** — pure-Rust wasm, entirely local; copy states this explicitly. |
| Live breach-database (HIBP) lookup with hit counts | **Out of model** — requires a network API call; gizza tools are offline/pure. Copy is explicit that this is a bundled list, not a live breach lookup. |
| Entropy / guessability score + time-to-crack estimate | **Out of model (this tool)** — that's a strength/entropy estimator, a distinct tool; we deliberately scope this one to the blocklist question and say so, pointing users to pair it with a strength check. |
| Pattern detection beyond leetspeak (keyboard walks, dates, repeats) | **Out of model** — belongs to a zxcvbn-style strength estimator, not a blocklist check. Keyboard-walk and repeat passwords that appear on the worst-list (e.g. `qwerty`, `111111`) are still caught by the list itself. |
| Composition suggestions ("add a symbol") | **Out of model** — this tool reports *why a password is weak*, not prescriptive tips; the message already steers users to long random passphrases. |
| UX presets / example chips | **In model** — meta.toml `[[example]]` chips: top common password, leetspeak variant, and a not-on-the-list passphrase. |

## Notes / honesty

- The single biggest differentiator competitors have (a live breach corpus) is
  intentionally out of model — gizza tools don't call external APIs. Rather than
  imply parity, the page, FAQ, chat description, and core doc-comments all state
  plainly that this is a **bundled blocklist / dictionary check, not a live breach
  lookup**, and that "not found" is not proof of strength.
- Entropy/time-to-crack is a legitimately separate tool; scoping is deliberate,
  not a gap silently dropped — every user-facing surface tells them to pair this
  with a strength/entropy check.
- No competitor wording or worst-password curation was copied; the bundled list is
  assembled from widely-published worst-password rankings.
