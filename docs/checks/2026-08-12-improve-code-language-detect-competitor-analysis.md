# code-language-detect — competitor analysis (2026-08-12)

Scan run BEFORE implementation, per `/create-next-tool` step 4. All notes are **paraphrased
observations** of publicly documented behaviour — no competitor copy, branding, or trademarks
were reproduced, and no competitor asset was used. Out-of-model items are listed, not built.

## What was searched

One web search for "online programming language detector / detect language of a pasted code
snippet". The reachable, genuinely comparable tools were skimmed below. Two search hits
(CodePal's detector, GetItFully's detector) returned 403/404 to the fetcher, so they were
replaced rather than run with fewer, and only their public search-result descriptions are noted
as secondary context.

## Competitor 1 — Creative Tech Guy "Code Detector & Formatter" (client-side utility)

- **Shape:** single paste box → an explicit "detect" action → detected language plus syntax
  highlighting/formatting of the snippet.
- **Params observed:** two unlabeled option dropdowns, plus a **"common languages only"**
  checkbox that narrows the candidate set to mainstream languages.
- **Architecture:** detection runs **in the browser** on top of a highlighting library — no
  upload, no account. Same shape gizza wants.
- **Stated guidance:** accuracy improves with more code; no documented max length or full
  language list.
- **Takeaway for us:** a "restrict the candidate set" control is a real, shipped feature, not
  decoration — short snippets are where detectors fail, and narrowing candidates is the cheapest
  fix a user can apply.

## Competitor 2 — Apify "Programming Language Detector" actor (hosted API)

- **Inputs:** raw source string, or a URL to a file. At least one is required; no defaults.
- **Coverage:** advertises 50+ languages spanning general-purpose languages, markup, config and
  data formats (JSON/YAML/XML/Markdown are treated as detectable "languages", not excluded).
- **Output fields:** detected language, a **confidence value in 0–1**, and an **`alternatives`
  array** of lower-confidence runner-up languages. Batch results export as JSON/CSV/etc.
- **Stated guidance:** recommends **3–5 lines minimum**; single-line snippets get low
  confidence; domain-specific languages are prone to misidentification.
- **Architecture:** server-side, metered per result (roughly cents per 100 snippets).
- **Takeaway for us:** confidence + ranked alternatives + an explicit short-snippet caveat are
  table stakes. The metering, the URL-fetch input and batch dataset export are the parts that
  do not fit a browser-local tool.

## Competitor 3 — guesslang (open-source ML detector, CLI + Python API)

- **Coverage:** 54 languages; model trained on ~1.9 M GitHub files, ~93% claimed accuracy on a
  230 k-file test set.
- **Options:** default output is the single top language name; a `--probabilities` flag prints
  the ranked candidates with percentages. Reads a file path or stdin. Custom model training is
  supported.
- **Stated limits:** explicitly warns that **very small snippets may be guessed wrong**, citing
  `print("Hello world")` as inherently ambiguous across languages.
- **Architecture:** TensorFlow model download at install time — decisively out of model for a
  wasm block.
- **Takeaway for us:** "top-1 by default, ranked probabilities on request" is the established
  CLI ergonomic, and the ambiguity warning belongs in the product, not just the docs.

## Reference — highlight.js `highlightAuto` (the library most web detectors sit on)

- Returns `language`, an integer **`relevance`** score, and a **`secondBest`** result object.
- Accepts a **`languageSubset`** array to restrict detection, settable per call or globally.
- Confirms the prevailing model: keyword/pattern relevance scoring, one point per matched
  signal, best-scoring grammar wins — i.e. a deterministic heuristic, no ML needed.

## Table stakes → where each one landed

| Table stake (seen in ≥1 competitor) | Fit | Where it landed |
| --- | --- | --- |
| Paste a snippet, get a language | in-model | `code` param (multiline textarea) |
| Confidence score (0–1) | in-model | `confidence` + a high/medium/low level |
| Ranked alternatives / probabilities | in-model | `top_k` (default 3, 0 = all scoring languages) |
| Restrict to a candidate subset (`languageSubset`) | in-model | `candidates` allowlist param |
| "Common languages only" toggle | in-model | `common_only` boolean |
| Filename / extension as a hint (Linguist's first strategy) | in-model | `filename` param, +10 score and named in the evidence |
| Shebang handling (`#!/usr/bin/env python3`) | in-model | generic shebang interpreter map, +10 |
| Machine-readable output for scripting | in-model | `output` = `report` \| `json` \| `language` |
| Short-snippet ambiguity warning | in-model | ambiguity notes + a stated limit on the page |
| Data/markup formats count as languages | in-model | JSON, YAML, TOML, XML, HTML, CSS, SCSS, Markdown, Dockerfile, Makefile are all detectable |
| Explanation of *why* (relevance evidence) | in-model | `explain` → the matched signals with weights and hit counts |
| Syntax highlighting of the pasted snippet | in-model but **rejected** | already shipped as `syntax-highlighter`; duplicating it here would fork the same feature across two tools |
| Upload a file / load from a URL | out-of-model | this is a paste-in pure block; the CLI/chat surfaces take text, not fetched files |
| Batch dataset export (CSV/Excel), per-result billing | out-of-model | no backend, no accounts, no metering |
| ML model with ~93% published accuracy | out-of-model | needs a TensorFlow model download; gizza blocks are pure Rust → wasm with no runtime download |
| Custom model training | out-of-model | see above; the closest gizza analogue is the separate `naive-bayes-text-classifier` |

## Design decisions taken from the scan

1. **Deterministic weighted-signal scoring**, in the highlight.js/Linguist tradition, rather than
   ML — it is the only approach that runs offline in wasm, and it makes the result explainable.
2. **Evidence is a first-class output.** No competitor shows *which* signals fired; that is the
   differentiator a heuristic detector can honestly offer over a black-box model.
3. **Filename beats prose.** Linguist checks the filename first; a supplied `filename` gets a
   strong bonus and is listed in the evidence so the user can see it swung the result.
4. **Say when it is a coin flip.** When the top two languages are within a small relative margin,
   or the snippet is under three lines, the tool says so instead of quietly returning a winner.
5. **Ambiguity is not an error.** With no matching signal the result is `unknown` at 0 %, with
   advice — not a thrown error.
