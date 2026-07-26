# persian-text-normalizer — competitor analysis (2026-07-26)

Scan done BEFORE implementation. Sources are open-source Persian NLP normalizers;
paraphrased only — no copy/branding reused.

## Competitors skimmed

1. **Virastar** (Ruby, brothersincode/virastar; Python port JKhakpour/virastar.py)
   — the canonical "cleaning-up Persian texts" normalizer with a live demo.
2. **Hazm normalizer** (roshan-research/hazm) — Persian NLP toolkit; its
   `Normalizer` is the most-cited affix/half-space handler.
3. **PersianNormalizer** (Ruby gem, hellboy2010) — "300+ unicode normalizations",
   focused on Arabic→Persian character folding (ي، ك، ؤ، آ …).
4. **Khoshnevis / PrePer** — add half-space (ZWNJ) correction and punctuation
   spacing on top of character folding.

## Table stakes (paraphrased, common across ≥2 competitors)

- **Arabic→Persian character folding.** ك (U+0643) → ک, ي (U+064A) → ی, ى
  (alef maksura U+0649) → ی. Some also map ة → ه. This is the #1 feature everyone
  ships.
- **Digit normalization.** Convert Arabic-Indic (٠١٢٣) and ASCII (0123) digits to
  Persian (۰۱۲۳). Virastar defaults English→Persian; most expose a direction toggle.
- **Half-space / ZWNJ (نیم‌فاصله).** Insert U+200C between a word and its attached
  affix — verb prefixes می/نمی, plural/comparative suffixes ها/تر/ترین/… — and
  clean stray spaces around existing ZWNJ. This is the hardest, most valued feature.
- **Punctuation spacing.** No space *before* . ، ؟ ! : ؛ and exactly one space
  *after* — the classic virastar spacing fix.
- **Remove diacritics / kashida.** Strip Arabic harakat (تشکیل, U+064B–U+0652,
  U+0670) and tatweel/kashida (ـ U+0640).
- **Whitespace cleanup.** Collapse repeated spaces, normalize line endings, trim.

## Optional / differentiating

- **Persian punctuation conversion** — Latin `,`→`،`, `?`→`؟`, `;`→`؛` (virastar
  offers it; off by default because it changes content).

## Defaults chosen (documented)

- `characters` = on (universally expected).
- `digits` = `persian` (this is a *Persian* normalizer; `english`/`keep` offered).
- `half_space` = on.
- `punctuation_spacing` = on.
- `persian_punctuation` = off (content-changing; opt-in, matches virastar).
- `remove_diacritics` = off (lossy; opt-in).
- `whitespace` = on.

## In-model vs out-of-model

- **In-model (built):** all six table-stakes above + optional Persian-punctuation
  conversion. Pure-Rust, deterministic, no deps — fits gizza's pure block model.
- **Out-of-model (not built):** tokenization / lemmatization / POS tagging (hazm's
  ML side), spell-correction, and emoji/URL stripping — those need models or are
  separate destructive cleaners, out of scope for a normalizer.

## UX controls

Checkboxes for the six on/off passes + a 3-way digit `<select>`, a multiline text
area, and preset example chips (messy paste, keep-Latin-digits, ZWNJ-only). Output
is the normalized text (flat string — model-compatible; no separate change count so
the chat/CLI/page surfaces share one string result).
