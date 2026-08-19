# recovery-code-generator competitor analysis — 2026-08-18

Backlog item: `recovery-code-generator` — "Generates BIP39 mnemonic phrases or one-time backup
codes for account recovery", pure compute, offline.

**Scope note (duplication guard).** The row names two different deliverables. The BIP39 half is
already shipped end-to-end by `blocks/bip39-mnemonic-generator` (12–24 words, checksum word,
`entropy_hex` for deterministic recovery, BIP39 passphrase, derived 512-bit seed) and validated by
`blocks/bip39-seed-derive`, so it is deliberately **not** re-implemented here — that would be the
same strict-subset duplication the skiplist rejects elsewhere. This block ships the half no
existing block covers: **one-time account-recovery / 2FA backup codes** — a numbered SET of
grouped, transcription-safe codes plus the SHA-256 digests a service stores instead of the codes.
The nearest shipped block, `blocks/random-token-generator`, draws single ungrouped tokens
(`length`/`count`/`charset`) with no block grouping, no separator, no sheet/CSV rendering and no
per-code storage digest, so the two do not overlap on the deliverable users actually want here.

## Sources skimmed

One WebSearch for the function ("backup codes generator 2FA recovery codes online tool"), then the
top real tools in the ranking were read directly. All observations below are paraphrased — no
competitor copy, naming, branding or trademark is reused anywhere in this block.

| Competitor | What it exposes | Table-stakes patterns observed | Fit decision |
| --- | --- | --- | --- |
| RandomKeygen — backup codes page | A single control: how many codes (8 / 10 / 12 / 16), rendering a numbered 1..N list. Default is 10 codes. "Copy all" and "download as text" actions. Guidance copy: print them, keep them in a password manager or encrypted container, never in email or plain cloud storage; each code is usually single-use, delete after use, regenerate ahead of time and immediately if exposed. | Count as the primary (often only) knob, with fixed preset counts; 10 as the default; a NUMBERED list rather than a bare blob; single-use + storage guidance next to the output. | **In-model.** `count` (1–50, default 10) and `output = numbered` (default) cover it; preset counts land as one-click example chips. Copy/download are platform features — the generated page already gets a copy button and, for `format = "text"`, a download link. |
| Genculator — 2FA backup codes generator | Four code shapes offered as formats: 4+4 alphanumeric with a dash, 5+5 alphanumeric, 4+4 numeric, and 4+4+2 numeric. Quantity choices 5 / 10 / 15 / 20 (default 10). Four "service style" presets named after large providers. Copy-all, text download, print/printable sheet. FAQ: what backup codes are, how many to keep (8–10), how to store them, how to pick a format, and that generation happens locally. | Grouped codes (blocks joined by a separator) as the defining shape; a numeric-only variant; provider-shaped presets; 5/10/15/20 quantities; printable output. | **In-model.** `blocks` × `chars_per_block` + `separator` express every one of those four shapes and more; `charset = numeric` covers the digit variants. Provider-shaped presets are reproduced as generic `[[example]]` chips describing the SHAPE (e.g. "8 digits, no separator"), never a provider's name or branding. Print is a browser function, not a block param. |
| Strong Random Password Generator — recovery code generator | Code length adjustable 8–32 characters, quantity 1–50, an optional "include special characters" toggle, an optional hyphen-formatting toggle "for readability", and a live "estimated entropy: N bits" readout. Download / copy / print / clear-from-memory actions. Copy: recovery codes are the fallback when the primary factor is lost; nothing is transmitted; store in a password manager, encrypted drive, or printed in a safe place; refresh after use and every 6–12 months; accepted formats vary per service. | Explicit code LENGTH range (8–32) and quantity range (1–50); optional hyphen grouping; an entropy readout shown with the codes; honest "formats vary by service" caveat. | **In-model.** Code length is `blocks × chars_per_block` (2–96 characters, so the 8–32 range is inside it); `count` caps at 50 to match; grouping is optional (blank `separator`, or `blocks = 1`); entropy per code and for the whole set is reported with every output. Symbols are deliberately out — see below. |
| `antonioribeiro/recovery` (PHP library, consulted for the API shape rather than as a UI) | Builder API: `setCount` (default 8), `setBlocks` (default 4), `setChars` per block (default 21… i.e. block size), `setBlockSeparator` (default `-`), and character-set selectors `numeric()` / `alpha()` / `lowercase()` / `uppercase()` / `mixedcase()`. Emits arrays, collections or JSON. Sample output looks like `C0r2Xp4o1v-oG3pteKXw3`. | The canonical parameter decomposition every server-side implementation uses — count, blocks, chars-per-block, separator, alphabet — plus machine-readable output (JSON) for scripted use. | **In-model, adopted verbatim as the parameter model.** `count` / `blocks` / `chars_per_block` / `separator` / `charset` map one-to-one, and `output = json` (plus `csv`) is the scripted equivalent of `toJson()`. |

## Descriptor decisions

Every table-stake above lands in the descriptor or in the out-of-model list below; none was dropped
silently.

- **`count`** — integer 1–50, default 10. The union of the observed quantity ranges (RandomKeygen
  8–16, Genculator 5–20, Strong Random 1–50); 10 is the shared default and the number every FAQ
  recommends.
- **`blocks`** — integer 1–6, default 2. Number of groups per code. `blocks = 1` produces an
  ungrouped code (the "no hyphens" toggle).
- **`chars_per_block`** — integer 2–16, default 5. `blocks × chars_per_block` is the code length, so
  the pair spans 2–96 characters and covers both the 8–32 length slider and the 4+4 / 5+5 / 4+4+2
  shapes.
- **`separator`** — string, default `-`, at most 3 characters, and rejected if it would collide with
  the alphabet (a separator that is also a code character makes a code ambiguous to read back).
  Blank means "no separator" even with several blocks.
- **`charset`** — `Param::enumv` over `lowercase` (default, `a–z0–9`), `alphanumeric` (mixed case),
  `uppercase`, `numeric`, `unambiguous` (letters+digits with `0/O/1/l/I` removed for printing and
  reading aloud), `hex`. This is the union of the library's alphabet selectors and the
  numeric-only variant the UI tools ship.
- **`output`** — `Param::enumv` over `numbered` (default), `plain`, `csv`, `json`. `numbered`
  reproduces the 1..N sheet all three UI tools render; `plain` is the paste-friendly list; `csv` and
  `json` are the scripted/importable equivalents of the library's `toJson()`.
- **`hash`** — `Param::enumv` over `none` (default), `sha256`, `sha256-salted`. **This is the
  capability gap in the competitor set**: every tool above hands the user codes and says "store them
  safely", but none produces what the *service* is supposed to store. A backup code must be kept
  hashed server-side exactly like a password, so the block can emit a SHA-256 digest per code (and a
  salted variant with a per-code random 16-byte salt, printed as `salt:digest`) alongside the codes.
- **`seed_hex`** — optional string, 8–128 hex characters. Blank (the default) means the codes come
  from the platform CSPRNG. When set, the code set is derived deterministically from that seed, which
  makes the tool reproducible for tests, for regenerating an identical sheet, and for the page's
  own deep-link verification. Mirrors the `entropy_hex` escape hatch `bip39-mnemonic-generator`
  already ships. The page copy states plainly that a seeded run is only as secret as the seed.

Entropy is reported for a single code and for the set, because two of the three UI tools show it and
because it is the only number that tells a user whether their chosen shape is strong enough.

## Deliberately not built (out of model, or covered elsewhere)

- **BIP39 mnemonic generation** — shipped by `blocks/bip39-mnemonic-generator` (and validated by
  `blocks/bip39-seed-derive`). Building it again here would duplicate an existing block.
- **Copy-to-clipboard, download, print, "clear from memory" buttons** — page-platform/browser
  concerns. The generated page already provides a copy control and a text download; printing is the
  browser's own function. None of these is a block parameter.
- **Provider-named style presets** ("<big provider> style") — the SHAPES are reproduced as generic
  example chips, but a competitor's or a third party's brand name is never used as a preset label.
- **Symbols / punctuation in the alphabet** — offered by one competitor as "special characters", but
  recovery codes are read off paper and typed into a login box, so punctuation raises transcription
  errors for a few bits; `unambiguous` addresses the same "make it typable" goal from the other
  direction. Users who want symbol alphabets already have
  `blocks/random-token-generator`'s `custom_chars`.
- **Code prefixes / per-service labels** — no competitor in this set exposes one, and a prefix adds
  zero entropy; a user can prepend one in their own notes.
- **QR export of the code sheet** — an image deliverable; `blocks/qr-code-generator` and
  `blocks/qr-paper-backup` already own that surface.
- **Bcrypt/Argon2 digests for storage** — `blocks/bcrypt-hash` and `blocks/argon2-hash` exist for
  slow password hashing. A high-entropy random code does not need a slow KDF (there is nothing to
  brute-force), so SHA-256 is the right and standard choice here; the page FAQ says so.

## Verification performed

Built and verified on branch `feat/tool-loop-20260806-001701`: `cargo test --workspace` in the
block, the locked `scripts/build-block-wasm.sh` artifact, the `wasm-pack` page bundle, the
generator render, the `gizza tool recovery-code-generator …` CLI surface (including an exact-output
seeded case and the page's own generated CLI example run verbatim), the advertised-values matrix
(every `charset` and `output` value, every `hash` value, the non-default booleans/enums, and the
`count` / `chars_per_block` cap boundaries at and one over), and the Playwright page spec
(real output plus a `?param=` deep link). `python3 scripts/check-tool-hygiene.py
recovery-code-generator` exits 0.
