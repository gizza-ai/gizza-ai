# base-decoder — competitor analysis (2026-07-10)

Scan for the "auto-detect / magic decode" category: tools that take an opaque
blob and figure out the encoding(s) without the user naming them. One WebSearch
(`CyberChef magic auto detect decode base64 base32 base58 tool online`) plus
category knowledge. All notes below are **paraphrased** — no competitor copy,
branding, or trademarks reproduced.

## Competitors skimmed

### 1. CyberChef "Magic" operation (gchq.github.io/CyberChef)
- Runs pattern-matching (regex per operation) + entropy in a background thread
  and *suggests* a recipe that would decode the current output; it does not
  auto-commit — the user clicks to apply.
- Knobs: **Depth** (how many layers deep to search), **Intensive mode** (brute
  more operations, slower), **Extensive language support**, and a **Crib**
  (a string the decoded output must contain, to disambiguate).
- Covers far more than bases (gzip, XOR, ciphers) because Magic sits on top of
  the whole recipe catalogue.
- Table stakes it demonstrates: multi-layer detection, a depth cap, showing
  *which* operations produced the result, and a preview snippet.

### 2. dCode "cipher/encoding identifier" (dcode.fr)
- Analyzes a string and lists candidate encodings/ciphers ranked by likelihood,
  each linking to its dedicated decoder.
- Detects Base64/32/16/58/85 among many classical ciphers.
- Presents *candidates* rather than a single committed answer.

### 3. Boxentriq / "Ciphey"-style auto-decoders
- Boxentriq's identifier ranks likely encodings and offers one-click decode.
- Ciphey (open-source CLI) recursively decodes through layers using a language
  checker to decide when it has reached plaintext — the closest analogue to what
  this tool does (recurse until natural text emerges).
- Table stakes: recursion until "looks like text", and a stop when a known file
  signature (magic bytes) appears.

## Table-stakes params / behaviors observed

| capability | seen at | in gizza model? | decision |
| --- | --- | --- | --- |
| Multi-layer / recursive decode | CyberChef, Ciphey | yes | **built** — core peels layers until text/signature/depth |
| Depth cap | CyberChef (Depth) | yes | **built** — `max_depth` (default 8, 1–30) |
| Show which encodings were applied | CyberChef, Ciphey | yes | **built** — report lists the detected chain (e.g. `base64 → base32`) |
| Stop at "looks like text" | Ciphey | yes | **built** — printable-ratio threshold accepts a text layer |
| Detect binary target via magic bytes | Ciphey, CyberChef | yes | **built** — PNG/JPEG/GIF/PDF/ZIP/gzip/zlib/ELF/… signatures stop the peel |
| Base16/32/45/58/64/85 alphabets | dCode, CyberChef | yes | **built** — all six, incl. URL-safe Base64 and Ascii85 |
| Plain vs. annotated output | (piping use) | yes | **built** — `output` = report \| plain |
| Crib / "must contain" filter | CyberChef | yes, but weak with greedy path | **considered, rejected** — a true crib needs a branch search; greedy path would give false negatives. Noted as out-of-scope on the page. |
| XOR / classical cipher detection | CyberChef, dCode | no (not a base encoding) | out of scope — this tool is base-encodings only |
| Language/entropy scoring beyond printable ratio | Ciphey, CyberChef | partial | printable-ratio heuristic only; documented limit |
| Candidate list (show N ranked guesses) | dCode, Boxentriq | possible | single best chain returned; documented — use the per-scheme codecs for ambiguous blobs |

## Design decisions

- **Auto-detect, single committed answer.** Unlike CyberChef (suggest-then-click)
  the gizza tool commits to the best chain and returns it, because chat/CLI want a
  direct answer. Ambiguity is documented (short/ambiguous blobs) and the
  per-scheme codec tools (base32-codec, base58-codec, base85-codec, multi-encoder)
  remain for manual control.
- **Descriptor params kept minimal + LLM-actionable:** `input` (required),
  `max_depth` (integer), `output` (enum report|plain). Every param has a
  `.describe()`; `output` is `Param::enumv`.
- **Whitespace/newlines ignored** for all bases except Base45 (space is a Base45
  data symbol).
- **Termination is guaranteed** — every base decode shrinks the buffer, so the
  peel always halts even before `max_depth`.
- **No competitor copy** used anywhere; all page copy is original.
