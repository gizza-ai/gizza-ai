# entropy-calculator — competitor analysis (2026-08-13)

Scan run BEFORE implementing, per `.claude/skills/create-next-tool`. All findings are
paraphrased observations of publicly visible tool surfaces; no competitor copy, wording,
branding, or trademark is reproduced or reused anywhere in this repo.

Backlog row: `entropy-calculator` (math, S, pure) — "Calculates Shannon entropy in bits of a
string or file to gauge randomness of keys, passwords, or data."

## Duplicate check (done first)

`ls blocks/ | grep -iE 'entropy|random|password|shannon|strength'` →

| Existing block | Scope | Overlap verdict |
| --- | --- | --- |
| `blocks/byte-entropy` | `Input::File` only (URL/attachment ref), Shannon entropy **per byte** over a file plus a per-block window series to spot encrypted/compressed regions. Chat + CLI, **no page**. | Complementary, not a dup. Different input modality (file bytes vs pasted text), different alphabet (256 byte values vs the text's own symbol set), different question ("where in this binary is the high-entropy region" vs "how many bits of information does this string carry"). `entropy-calculator` therefore ships **text-first with a page**; file/binary entropy stays with `byte-entropy` and is cross-referenced in the page copy. |
| `blocks/password-entropy` | Strength estimate from the **assumed character pool** (`length × log2(pool)`), rating, crack time, weakness flags. | Not a dup: that is the pool/guessability model, deliberately *not* Shannon entropy of the observed string. The two answer different questions and disagree on purpose (see the FAQ entry on the page — `password` scores ~47 pool bits but only ~2.75 Shannon bits/char × 8). |
| `random-token-generator`, `password-generator`, `weak-password-detector`, … | Generators/detectors. | Unrelated. |

Sibling backlog rows that remain distinct and are intentionally **not** absorbed here:
`distribution-entropy-analyzer` (Shannon/Rényi/Tsallis + perplexity over a *probability or
count vector*), `series-entropy-calculator` (sample/permutation entropy of a numeric series),
`image-entropy-calculator` (intensity histogram of an image).

Conclusion: **build**, scoped to text/symbol-sequence entropy.

## Competitors reviewed

1. **onlinetexttools — calculate text entropy** (`onlinetexttools.com/calculate-text-entropy`)
2. **DevToys Web Pro — entropy calculator** (`devtoys.pro/calculators/entropy`)
3. **calcexp — Shannon entropy calculator** (`calcexp.com/math-science-calculators/shannon-entropy-calculator/`)
4. **dCode — Shannon index** (`dcode.fr/shannon-index`)

(A fifth class — generic "text entropy analyzer" SEO pages such as easyprotools/freetoolscorner —
was skimmed and adds nothing beyond #1/#2: paste box, bits/char, unique-character count.)

## Feature matrix

| Capability | 1 onlinetexttools | 2 DevToys | 3 calcexp | 4 dCode | gizza decision |
| --- | --- | --- | --- | --- | --- |
| Paste text → entropy | ✅ | ✅ | ✅ | ✅ | **in-model** — `text` (required, multiline) |
| Entropy per symbol (bits/char) | ✅ | ✅ | ✅ | ✅ | **in-model** — headline number |
| Total information (bits/char × length) | — | ✅ | — | — | **in-model** — `total` line |
| Log base / unit: bits, nats, dits(hartleys), trits | — | — | ✅ | bits only | **in-model** — `unit` enum `bits\|nats\|dits\|trits` |
| Symbol basis (what counts as a symbol) | characters | characters | characters | characters | **in-model, extended** — `basis` enum `characters\|bytes\|words\|lines`; `bytes` = UTF-8 bytes (0–8 bits/byte, the byte-entropy convention), `words`/`lines` cover corpus/log analysis none of the four offer |
| Per-line / per-paragraph entropy | ✅ (3 modes) | — | — | — | **in-model** — `scope` enum `whole\|line\|paragraph` |
| Adjustable decimal precision | ✅ | — | — | — | **in-model** — `precision` 0–10, default 4 |
| Maximum entropy (log_b of distinct symbols) | — | — | ✅ | — | **in-model** — reported |
| Efficiency % / redundancy % | — | — | ✅ | — | **in-model** — both reported |
| Perplexity (b^H) | — | — | ✅ | — | **in-model** — reported |
| Distinct-symbol count | — | ✅ | — | — | **in-model** — reported |
| Character-frequency distribution | — | ✅ (chart) | — | — | **in-model as a text table** — `show_frequencies` + `top_symbols`; ASCII bar column stands in for the chart (this repo's page output is text, not canvas) |
| Case-insensitive folding | — | — | — | — | **in-model** — `ignore_case` (asked for constantly when comparing English text against the ~4.1 bits/letter reference figure) |
| Ignore whitespace | — | — | — | — | **in-model** — `ignore_whitespace` |
| Worked examples / presets | ✅ (3 examples) | ✅ (1 example) | ✅ (die, English, LLM) | ✅ (5-letter word) | **in-model** — 4 `[[example]]` preset chips + a worked example in the page copy |
| Frequency-vector input (counts / probabilities instead of text) | — | — | ✅ | ✅ | **deferred, not dropped** — this is exactly the scope of the separate unbuilt backlog row `distribution-entropy-analyzer` (which also adds Rényi/Tsallis). Building it here would pre-empt and duplicate that row. Recorded here so it is not silently lost. |
| File upload | — | — | — | — | **already shipped elsewhere** — `blocks/byte-entropy` takes a file URL/attachment and adds a per-block entropy series. The page copy points at it rather than duplicating a file input (this repo has no generic pure-wasm file input on pages — only ffmpeg/model runtimes get one). |
| Rényi / Tsallis entropy | — | — | — | — | **out of scope** — `distribution-entropy-analyzer` row. |
| Sample / permutation entropy of numeric series | — | — | — | — | **out of scope** — `series-entropy-calculator` row. |
| Save to file / clipboard / pastebin export | ✅ | ✅ (clear) | — | — | **already generic** — the page generator gives every `format = "text"` tool Copy + Download + Reset for free. |
| Conditional / n-gram (order-N) entropy | — | — | — | — | **not built** — genuinely in-model math, but it changes the tool's claim from "Shannon entropy of the symbol distribution" to a language-model estimate and needs its own copy/limits; noted as a future enhancement, not shipped, so the headline number stays the one every competitor agrees on. |

## UX patterns adopted

- **Preset chips** (`[[example]]`) for the four canonical demos: a low-entropy repeated string, an
  API-key-style token, English prose vs. per-line comparison, and a bytes-basis run. Competitors 1,
  3 and 4 all lead with worked examples; chips are this repo's declarative equivalent.
- **Slider** (`kind = "slider"`) for `precision` (0–10) and `top_symbols` (0–64) — bounded numeric
  ranges, per `references/page-patterns.md`.
- **Friendly `<select>` labels** (`[input.labels]`) for `basis`, `unit`, and `scope` so the enum
  values stay canonical for the CLI/chat while the page reads in plain English.
- **`multiline = true`** on `text` so pasted multi-line input survives (required for `scope = line`
  and `scope = paragraph` to be usable at all).
- **`wide = true`** — the report has a frequency table column.

## Copy / SEO gaps closed

- A worked example with **both input and exact output** (competitor 1 and 4 both do this; two of
  the four bury the formula with no numbers).
- Stated **limits** on the page, not just in code (1 MB input cap, 20k lines, symbol-table cap).
- FAQ answers the three questions every competitor gets asked and none answer together: why this
  number differs from a password-strength estimate, why short strings over-report per-symbol
  entropy, and what unit to use when.

## Honest limitations recorded on the page

- Shannon entropy of an *observed* string is a property of that string's symbol distribution; it is
  not a guessability estimate for a password (that's `password-entropy`) and not a proof of
  cryptographic randomness.
- Order-0 model: `abababab` and `aabbaabb` score identically. Stated explicitly.
- Short inputs cannot exceed `log_b(length)` bits per symbol, so a 4-character key maxes out at 2
  bits/char no matter how random its source.
