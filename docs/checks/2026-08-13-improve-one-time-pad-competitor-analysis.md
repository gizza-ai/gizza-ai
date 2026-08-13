# one-time-pad — competitor analysis (2026-08-13)

Scan run **before** implementing, per `create-next-tool` step 4. All findings are paraphrased
observations of publicly visible tool behaviour — **no competitor copy, branding, or trademarks are
reproduced or reused anywhere in this block**. Out-of-model items are listed, not built.

## Why this is not a duplicate of an existing block

Checked before scaffolding (`ls blocks/ | grep -iE 'pad|xor|cipher|otp|encrypt|random'`):

- **`blocks/xor-cipher`** — repeating-key bytewise XOR (`byte ^ key[idx % key.len()]`, see
  `blocks/xor-cipher/core/src/lib.rs:83`). It deliberately *repeats* a short key, which is exactly
  the anti-pattern a one-time pad forbids; it has no pad generation and no length enforcement.
- **`blocks/classical-cipher-tool`** — Caesar / Vigenère / Atbash / rail-fence with a repeating
  keyword. Vigenère is the repeating-key relative of an OTP, not an OTP: no matching-length random
  pad, no mod-10 digit mode.
- **`blocks/random-token-generator`** — CSPRNG tokens by charset/length, but not sized to a message
  and not wired into an encrypt/decrypt round trip.

Distinct capabilities this block adds: **matching-length CSPRNG pad generation**, **strict
pad-length enforcement** (the defining security property — a short pad is rejected, never
repeated), and **mod-26 letter / mod-10 digit modular arithmetic** alongside XOR.

Viability of "truly random pad": confirmed in-model. `getrandom` 0.2 is already proven across
surfaces in this repo (`blocks/random-token-generator/core/src/lib.rs` — WASI `random_get` on
`wasm32-wasip1`, the `js` backend on the page's `wasm32-unknown-unknown` build). Encryption and
decryption stay fully deterministic given a supplied pad, so the page/CLI/spec all assert exact
output; only the generate path is random, and it is asserted on shape + round trip.

## Competitors reviewed

| # | Tool | What it does |
|---|------|--------------|
| 1 | dcode.fr — Vernam cipher | Splits the OTP into two implementations: Vernam/Vigenère (random key as long as the message) and Vernam/XOR (random binary key matching the plaintext bit size). Export as text, copy/paste. |
| 2 | boxentriq.com — One-Time Pad (Vernam) | Key-stream field with a live key-length check, encrypt/decrypt mode, alphabet selector; letter alphabets use `C = (P + K) mod m`, binary alphabets use XOR; preserves spacing, punctuation and letter case; step-by-step per-character panel and a tabula recta reference; copy + download. |
| 3 | devoven.com — One-Time Pad Generator (XOR) | "Encrypt (generate key)" vs "Decrypt" toggle; auto-generates the key during encryption; key and ciphertext both shown as hex; copy / save-to-file / share; notes on its own page that its RNG is `Math.random()` and therefore **not** cryptographically secure. |
| 4 | thisdevtool.com — One-Time Pad (OTP) Cipher | Three tabs: Encrypt / Decrypt / Generate Key; "Generate New Key" auto-adjusts to match the message length; hex key + hex ciphertext; Try Example / Clear All; worked walkthrough `HELLO` + `XMCKA` → `EQNVO` (modular addition); 5-question FAQ. |
| 5 | cachesleuth.com — One Time Pad Cipher | Vigenère-style pad "where the key must be at least as long as the message"; random-key button; heavy text-prep options (case, filters, whitespace, find/replace) and configurable **grouping at character intervals**. |

## Table stakes → decision

| Capability | Seen in | In model? | Where it landed |
|---|---|---|---|
| Encrypt / decrypt with a supplied pad | 1,2,3,4,5 | ✅ | `mode = encrypt\|decrypt` |
| Generate a random pad on its own | 3,4,5 | ✅ | `mode = generate-pad` |
| Auto-generate the pad while encrypting | 3,4 | ✅ | `mode=encrypt` with an empty `pad` → fresh pad generated and returned alongside the ciphertext |
| Pad sized to the message automatically | 3,4 | ✅ | `length = 0` (default) derives the pad size from `message` for the selected cipher |
| Letter alphabet, `C = (P + K) mod 26` | 1,2,4,5 | ✅ | `cipher = letters` (default) |
| Bytewise XOR with a hex pad | 1,2,3,4 | ✅ | `cipher = xor`, pad + ciphertext in `encoding = hex` (default) |
| Digit alphabet, mod 10 | (classic numeric OTP practice; 2's "custom alphabets") | ✅ | `cipher = digits` |
| Base64 pad/ciphertext as well as hex | (3,4 are hex-only) | ✅ | `encoding = hex\|base64` — a small superset, no competitor copy involved |
| Pad must be at least as long as the message; live length check | 2,5 | ✅ | Hard error naming the shortfall, e.g. `pad too short: message needs 12 pad letters, pad has 5`. Never silently repeats. |
| Preserve spaces / punctuation / case; non-alphabet chars consume no pad | 2 | ✅ | Default behaviour of `letters` and `digits` |
| Grouping the output into fixed-size blocks | 5 | ✅ | `group = 0..20` (0 = keep the original layout); `group=5` gives classic five-character traffic groups |
| Cryptographically secure RNG | 4 (3 explicitly is not) | ✅ | `getrandom` (WASI `random_get` / WebCrypto), rejection sampling for the non-power-of-two alphabets so there is no modulo bias |
| Copy result / reset / download | 2,3,4,5 | ✅ | Provided by the shared page runtime for `format = "text"` — no per-tool code |
| Preset one-click examples | 4 ("Try Example") | ✅ | `[[example]]` chips in `page/meta.toml` |
| Deep-linkable parameters | 3 ("copy URL with parameters") | ✅ | The page generator prefills + auto-runs from `?param=` — covered by a Playwright case |

## Deliberately NOT built (out of model / out of scope) — listed, not dropped

- **Tabula recta reference table and a per-character step-by-step derivation panel** (2). This is a
  teaching visualisation, not a computation; rendering it needs bespoke page JS (`page/custom.js`)
  for what the FAQ explains in two sentences. The worked example in `page/content.md` shows the
  arithmetic for the canonical vector instead.
- **QR code of the pad, social share buttons, save-to-file beyond the shared Download link** (3).
  Sharing/branding surfaces; QR generation is already `blocks/qr-generate`'s job, and chaining is
  better than duplicating an encoder here.
- **Keyword-derived custom alphabets** (keyword position, reversal, last-instance dedup) (5). That
  is classical-cipher alphabet construction and belongs to `blocks/classical-cipher-tool`, not to a
  pad tool; an OTP's security comes from the pad, not from a scrambled alphabet.
- **Bulk text-prep options** — case folding, vowel/consonant filters, find-and-replace, newline
  normalisation (5). Already covered by existing text blocks (`blocks/text-case`,
  `blocks/find-replace`-class tools); duplicating them here would make the pad tool a text editor.
- **A pad "notebook"/multi-page pad with used-page tracking.** Stateless block model — there is no
  per-user storage in this repo to record which pad pages were spent.

## Verification performed

`cargo test --workspace`, `scripts/build-block-wasm.sh one-time-pad`, `wasm-pack` browser build,
`cargo install --path cli` + `python3 scripts/sync-tool-manifest.py one-time-pad`, the page
generator, CLI runs including one exact-output case, the Playwright page spec (every `cipher`
choice, both `encoding` values, the auto-generate path, a `?param=` deep link, the pad-too-short
error, and the `group` boundary), and `python3 scripts/check-tool-hygiene.py one-time-pad`.
