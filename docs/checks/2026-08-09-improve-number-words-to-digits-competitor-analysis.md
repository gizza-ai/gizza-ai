# number-words-to-digits — competitor analysis (2026-08-09)

Scan done BEFORE implementing. One web search (`words to numbers converter tool spelled out
numbers to digits online`) plus a direct skim of the top reachable competitor tools. All notes are
paraphrased observations of *functionality*; no competitor copy, branding, or trademarks are
reproduced or reused anywhere in this tool.

## Tools skimmed

| # | Tool | Reachable | Notes |
|---|------|-----------|-------|
| 1 | onlinetools.com — words → numbers converter | yes | Deepest feature set of the set: decimal words, named fractions, digit-by-digit, multi-line batch, preset example chips, `?input=` deep link |
| 2 | dcode.fr — writing words → numbers | yes | Language selector (EN/FR), output thousands-separator convention (none / thin space / comma), large-scale table, cardinal-vs-ordinal explainer |
| 3 | coolconversion.com — words to numbers converter | yes | Single field, "Convert"/"Copy" buttons, comma-grouped output, states a magnitude limit (999 trillion), quick-access preset links |
| 4 | bookmarked.tools — convert words to numbers | yes | Minimal: one field, convert/clear, "processes in your browser" privacy framing; no options documented |
| — | codeshack.io — words to numbers converter | **no (HTTP 403)** | Replaced by #4. Search result described lakh/crore scale support, which is recorded below as a table-stake anyway |

## Table-stakes observed → decision

| Capability | Seen on | In model? | Decision |
|---|---|---|---|
| Cardinal words → digits (units, teens, tens, hundred, thousand … trillion) | all | yes | Core parser, always on |
| `and` connector (`one hundred and forty-two`, `one thousand and one`) | 1, 2, 4 | yes | Accepted, but only where English actually uses it (after `hundred`/a scale word, or before a fraction) so prose like `one and two` is not silently merged |
| Hyphenated compounds (`twenty-five`, `forty-seven`) | all | yes | Tokenizer splits on hyphens/en dashes |
| Scales beyond trillion (quadrillion … decillion) | 1, 2 | yes | Supported to decillion (10^33); anything past the 128-bit range errors clearly |
| Long-scale (European) reading of billion/trillion + `milliard` | 2 (FR mode) | yes | `scale` param: `short` (default) / `long` |
| Indian scales `lakh` / `crore` | codeshack (search result) | yes | Always accepted; they do not collide with short/long scale words |
| Negatives (`minus one hundred`) | 1 | yes | `minus` / `negative` prefix, always on |
| Decimal words (`five point forty-seven` → 5.47) | 1 | yes | `point` / `decimal` / `dot`, always on |
| Fraction words (`one and a half` → 1.5, `six and a quarter` → 6.25, `three quarters`) | 1 | yes | `fractions` param (default on), limited to halves/quarters — see out-of-scope |
| Digit-by-digit runs (`one two three` → 123) | 1 | yes | `digit_sequences` param (default off, because it changes the meaning of ordinary prose) |
| Multi-line / batch input, one result per line | 1 | yes | Multiline textarea; `value` and `extract` modes work line by line |
| Convert numbers embedded in prose, keep the rest of the text | 1 (partly) | yes | `mode = replace` (default) — the differentiator most of the field lacks |
| Output thousands separator choice | 2, 3 | yes | `separator` param: `none` (default, machine-readable) / `comma` / `space` / `underscore` |
| Ordinals (`twenty-first`) | 2 (explainer only) | yes | `ordinals` param: `cardinal` (default) / `suffix` (`21st`) / `ignore` |
| Preset example chips | 1, 3 | yes | Four `[[example]]` chips on the page |
| URL deep link to prefilled input | 1 | yes | Generator gives `?param=` deep links for free; covered by a Playwright case |
| Copy button / privacy framing (runs locally) | 1, 3, 4 | yes | Generator page ships a copy control; everything runs in-browser/CLI, no network |
| Stated magnitude limit | 3 | yes | Documented on the page: exact integers to ~1.7 × 10^38, input capped at 200,000 characters |

## Explicitly out of scope / out of model (listed, not built)

- **Currency word parsing** (`one dollar and fifty cents`, coin names like nickel/dime, and the
  currency-formatted output styles onlinetools offers). Different tool: money *formatting* is
  already `blocks/number-to-currency-formatter`, and coin-name lexicons are locale data, not
  number parsing. Not built here.
- **Non-English input** (dcode's French mode). Would need a per-language lexicon and grammar
  (agreement, `quatre-vingts`, `soixante-dix`). Out of scope for this English parser; a separate
  block would be the honest shape.
- **Multiplicative phrases** (`six sixes` → 36, `twelve hundreds` → 1200, seen on onlinetools).
  Ambiguous against ordinary prose (`the twelve hundreds` is a decade) and would misfire badly in
  the default in-prose replace mode. Not built.
- **Thirds/fifths/eighths as fractions.** Those words are also ordinals (`the third row`), so
  accepting them as fractions would corrupt ordinal handling; and `one third` has no exact decimal
  form. Only halves and quarters — which are never ordinals — are treated as fractions.
- **Indian digit grouping of the output** (`12,34,567`). The separator choice here is the
  character only; lakh/crore *input* is supported. Indian output grouping already exists in
  `blocks/number-to-currency-formatter`.
- **Roman numerals, `googol`, and arbitrary-precision scales past 10^33.** Beyond the exact
  128-bit integer range this block guarantees; the block raises a clear "number is too large"
  error rather than silently losing precision to floating point.

## Verification notes

CLI exact-output checks, page Playwright checks (real output plus a `?param=` deep link), the
advertised-values matrix (every enum choice, both non-default booleans, and the exact input cap
boundary), `wasm-pack`, manifest sync, and the hygiene gate were all run for this build. The page
generator rendered this tool page successfully; the full repository-wide generator command is slow
on this continuation box and hit the 10-minute foreground-command limit after rendering through the
late alphabet, so its per-tool output was verified via the generated page artifacts and Playwright.
