# password-generator — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/password-generator` — generate strong random passwords or
word passphrases. Chat + CLI + page. Pure-Rust, `getrandom` CSPRNG.

## What competitors do

- **Online password generators** (1Password/LastPass generators, random.org,
  many "strong password" sites) — pick length/options, copy. The big trust issue:
  some generate **server-side** (you must trust they don't log it); the good ones
  generate client-side. Several are ad-heavy or bundle a manager upsell.
- **`openssl rand` / `pwgen`** — local + scriptable but CLI-bound and limited
  options.

## How this tool competes / improves

1. **Generated locally with a CSPRNG.** Pure-Rust + `getrandom` (OS/WASI entropy)
   compiled to wasm: page in-browser, CLI headless, chat Service Worker. The
   secret never leaves the device.
2. **Unbiased randomness.** Character/word indices use **rejection sampling**, so
   there's no modulo bias toward certain characters — a correctness detail many
   naive generators (`rand() % n`) get subtly wrong.
3. **Passwords *and* passphrases.** Random-character passwords (length + toggle
   uppercase/digits/symbols, lowercase always on) or memorable word passphrases
   (word count + separator) in one tool.
4. **Reports entropy bits**, so you can see how strong the result actually is
   (e.g. a 16-char full-alphabet password ≈ 100+ bits).
5. **Three surfaces + deep-links** (great for scripting/CI password generation
   via the CLI, or a quick page).

## Honest scope

- Passphrase wordlist is a built-in ~120-word list (~6.9 bits/word) chosen for
  short, typeable words — fine with enough words; a full EFF/diceware list (much
  larger) would be a future upgrade for higher per-word entropy.
- Password alphabet is fixed per class (symbols = a common safe set); no custom
  "exclude ambiguous characters" toggle yet.

## Tests

5 core unit tests: password has the requested length and only uses the allowed
charset, with >100 bits for 20 chars over the full alphabet; lowercase-only when
all class flags are off; two generations differ; passphrase has the requested
word count + separator and words from the list; error cases (length 0/too big,
words 0/too big). Plus the block drift-guard schema test. CLI + Playwright
(password via fill; passphrase via deep-link) verified — see commit. (Random
output is tested by structure — length/charset/word-count/entropy — not exact
value.)
