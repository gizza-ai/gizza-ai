# urdu-romanizer — competitor analysis (2026-09-04)

Scan run **before** implementation, per the create-next-tool procedure. One web search
("Urdu to Roman Urdu transliteration online tool") plus direct reads of the top real
competitor tools. Everything below is **paraphrased** — no competitor copy, branding, or
trademarks were reproduced, and none of their wording was reused in the page.

## Competitors examined

| # | Tool | Reachable | What it is |
|---|------|-----------|------------|
| 1 | iJunoon — Urdu to Roman transliteration | yes (thin landing page) | Single-box Urdu → Roman converter, part of a bidirectional transliteration section |
| 2 | mylanguages.org — Urdu romanization | yes | Single textarea phonetic romanizer with a hard character cap and a separate "cleanup" step |
| 3 | TranslatorMind — Urdu to Roman Urdu | yes | AI/LLM-backed converter, single box, larger character cap |
| — | easyurdutyping.com | **no (404)** | Replaced by TranslatorMind, per the "replace an unreachable competitor" rule |
| — | Wikipedia: Roman Urdu | yes (reference, not a tool) | Used only to ground the named romanization standards |

### 1. iJunoon (ijunoon.com/transliteration/urdu-to-roman/)
- Features: paste Urdu script → Roman Urdu; a sibling page does the reverse direction.
- Params/options: none exposed on the page.
- Limits: none stated.
- UX: one input area, one action; no presets, no documented copy/download, no worked example.
- SEO angles: "Urdu to Roman", "نقل حرفی", bidirectional transliteration hub.

### 2. mylanguages.org (urdu_romanization.php)
- Features: phonetic romanization of pasted Urdu.
- Params/options: none; no scheme selector, no documented letter table.
- Limits: **750 characters** stated explicitly.
- UX: three-step workflow — convert, copy, then run the result through a separate
  "cleanup" page to reduce errors. Minimal single-textarea interface.
- Notable: openly frames output as needing manual post-editing; no transparency about
  which romanization system it follows.

### 3. TranslatorMind (urdu-to-roman-urdu-translator)
- Features: Urdu script → phonetic Roman Urdu; states it keeps punctuation, emoji, and
  line breaks intact.
- Params/options: none exposed.
- Limits: **3,000 characters**.
- UX: input box, convert action, copy, clear, feedback widget; "how it works" section,
  use-case list (messaging, social posts, pronunciation practice).
- Accuracy: carries a disclaimer that output is AI-generated and may be imperfect.
- Free/paid: free, but server/AI-backed (requires a round trip).

### 4. Reference — named romanization standards (Wikipedia, Roman Urdu)
- Formal systems in circulation: **ALA-LC**, **ISO 15919**, Hunterian, ArabTeX,
  Uddin & Begum.
- Recurring criticism of ad-hoc Roman Urdu: it is not reversible to Urdu script and
  under-specifies pronunciation.
- Documented ambiguity sources: choti he `ہ` vs do-chashmi he `ھ`; the digraph `sh`
  (from `ش`) vs `س` + `ہ`; `zh` (from `ژ`) vs `ز` + `ہ`.
- Informal Roman Urdu as used online restricts itself to the 26 ASCII letters, no
  diacritics.

## Table-stakes extracted, and where each landed

| Table stake | Source | In/out of model | Decision |
|---|---|---|---|
| Paste Urdu script → Roman output | all 3 | in-model | `text` param, multiline textarea |
| Preserve punctuation and line breaks | #3 | in-model | `punctuation = keep` mode; line breaks preserved in all modes |
| Convert Urdu punctuation to Latin | implicit in all | in-model | `punctuation = latin` (default): `۔ ، ؟ ؛ ٪` → `. , ? ; %` |
| Digit handling (`۰-۹`) | none stated it | in-model | `digits = latin` (default) / `keep` — a gap none of the three documented |
| Copy result / reset | #3 | in-model | provided by the shared page chrome (Copy result + Reset), no per-tool code |
| Stated character limit | #2 (750), #3 (3,000) | in-model | no artificial cap — this runs locally; the page states that instead |
| Named romanization scheme | none offered a choice | in-model | `scheme = informal / ala-lc / iso15919` — the clearest differentiator |
| Sentence casing of output | none | in-model | `capitalization = none / sentence / title`, default `sentence` |
| Short-vowel restoration (script omits them) | #3 does it with a model | **partly out-of-model** | deterministic `short_vowels = insert-a / marks-only / omit` + honours zabar/zer/pesh when present; a common-word dictionary (`common_words`) covers the highest-frequency words |
| Izafat (`-e-`) linkage | none | in-model | word-final kasra renders `-e` (informal) / `-i` (scholarly) |
| Reverse direction (Roman → Urdu) | #1 | in-model but out of scope | separate tool; not folded into this slug |
| Server/AI translation quality | #3 | **out-of-model** | gizza runs local wasm, no account, no backend — listed, not built |
| Post-conversion "cleanup" second page | #2 | rejected on judgment | a second manual pass is a workaround for an opaque converter, not a feature |
| Hunterian / ArabTeX / Uddin-Begum schemes | Wikipedia | in-model, deferred | the three shipped schemes cover the documented demand; adding more without a published table to verify against would be inventing a standard |

## UX control patterns adopted

- Enum params render as native `<select>`s with friendly labels (`[input.labels]`), so the
  scheme/vowel/digit/punctuation/case choices are discoverable rather than hidden.
- `[[example]]` preset chips for the four representative runs (default informal, diacritics
  honoured, ALA-LC scholarly, ISO 15919) — competitors ship zero presets; chips are this
  repo's declarative preset answer.
- `multiline = true` on the text field so pasted paragraphs and line breaks survive.
- Shared Copy result / Reset buttons instead of a bespoke toolbar.
- Limits and the exact letter table stated on the page, not discovered through an error —
  directly answering competitor #2's opacity about its own scheme.

## Out-of-model (considered, not built)

- Neural/statistical short-vowel restoration and word-sense disambiguation (competitor #3's
  approach) — needs a model and a server round trip.
- Roman Urdu → Urdu script (the reverse direction) — a distinct tool, not a mode here.
- Account-gated history, feedback collection, and usage analytics.
- Server-side batch/file upload conversion.

## Honest limits recorded on the page

- Urdu script does not write short vowels; without diacritics the output is an
  approximation (`کتاب` → `katab`, not `kitab`) unless the word is in the common-word list
  or the input carries harakat.
- Informal ASCII output is lossy and not reversible: `ت/ط`, `س/ص/ث`, `ز/ذ/ض/ظ`, and
  `ٹ/ت` all collapse. The `ala-lc` and `iso15919` schemes keep those distinctions.
- `خ` and `ک`+`ھ` both render `kh` in informal mode (a documented Roman Urdu ambiguity);
  the scholarly schemes disambiguate.
