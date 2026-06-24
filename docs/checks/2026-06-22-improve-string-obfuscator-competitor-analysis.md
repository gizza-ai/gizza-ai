# string-obfuscator — competitor analysis (2026-06-22)

Tool: `blocks/string-obfuscator` — mask or visually obfuscate a string for
screenshots, demos, and docs. Modes: `mask`, `rot`, `leetspeak`, `homoglyph`.
Built new this run; analysis informs the v0.1.0 feature set. All findings are
paraphrased — no competitor copy, branding, or assets were reused.

## Surfaces verified (Phase 1, post-build)

- **Chat / LLM API:** `cargo test --workspace` green (10 core + 1 drift-guard
  schema test); wafer fixtures `mask.json`, `rot13.json`, `leetspeak.json`,
  `bad-mode.json` build into the validated `target/block.wasm`.
- **CLI:** `gizza tool string-obfuscator …` verified for all four modes plus the
  invalid-mode error (exit 1). e.g. `mode=mask keep_start=5 keep_end=4` on
  `sk-1234567890abcdef` → `sk-12**********cdef`.
- **Page + query-params:** Playwright (4 tests) covers mask, rot13 via the
  `<select>`, leetspeak, and the `?text=…&mode=rot` deep-link prefill+compute.

## Competitor landscape (paraphrased)

Five representative classes of existing tools cover this function; there is no
single dominant "string obfuscator" — the space splits across redaction and
toy-cipher tools:

1. **Online masking / redaction tools** (generic "mask string" utilities).
   Pattern: keep first/last N characters visible, replace the middle with a
   chosen symbol. Some offer email-aware masking (keep the first letter and the
   domain). Output is a single masked string. UX: a text box + a couple of
   numeric "visible characters" fields.
2. **ROT13 / Caesar-cipher tools.** Pattern: a fixed ROT13 toggle, sometimes a
   shift slider for arbitrary ROT-N. Often paired with a "this is not encryption"
   disclaimer. Self-inverse ROT13 is the headline.
3. **Leetspeak / "1337" translators.** Pattern: letter→digit/symbol substitution
   with varying aggressiveness (basic a/e/i/o/s vs. heavy multi-symbol). Mostly
   novelty / gaming.
4. **Unicode homoglyph / "confusable" generators.** Pattern: map ASCII letters to
   look-alike codepoints (Cyrillic/Greek/math) so text looks identical but is a
   different byte string. Framed around homograph-spoofing demos and copy-paste
   evasion.
5. **Combined "text obfuscator" toys** that bundle reverse / zalgo / upside-down /
   case-randomize as a grab-bag of transforms.

## Gap diff vs. our v0.1.0 (fit-to-model)

In-model gaps we **closed** in the initial build (so v0.1.0 ships at parity):

- Configurable mask character + independent keep-first / keep-last counts
  (covers class 1's core feature, including the email-shape case via whitespace
  preservation).
- Arbitrary ROT-N shift, not just a fixed ROT13 (covers class 2 fully), with the
  self-inverse default highlighted and a "not encryption" note in the copy.
- Leetspeak substitution (class 3).
- Unicode homoglyph mode with an explanation of homograph spoofing (class 4).
- Correct Unicode-scalar counting (multi-byte input masks correctly) — a common
  bug in naive byte-based maskers.

Out-of-model / considered, **not built** (documented, not forced in):

- Server-side batch/file upload masking — gizza is browser-local; the page
  already does unlimited local input, so no backend is warranted.
- Email-/regex-aware *auto-detection* of what to mask — that is the distinct
  `redact-pii` tool's job (auto-detects emails/phones/IPs/cards/SSNs); kept
  separate to avoid a semantic dup. This tool is intentionally user-driven.
- "Grab-bag" novelty transforms (zalgo, upside-down, reverse) from class 5 —
  low utility for the masking/redaction use case; out of scope for v0.1.0.
- Bidirectional homoglyph *decode* — homoglyph and leetspeak are intentionally
  one-way (lossy) here; only ROT is reversible.

## Result

v0.1.0 ships at or above feature parity with each competitor class for the
in-model surface, with original copy/UX and the no-copy rule observed. No
further in-model gaps outstanding.
