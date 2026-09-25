# regex-from-examples — competitor analysis (2026-08-29)

Scan run **before** implementation, per `/improve-tool` Phase 2–3, to set the table stakes for the
new `regex-from-examples` block. All observations are **paraphrased** from public documentation and
product pages; no competitor copy, branding, naming or assets are reused anywhere in this tool.

## Top 3 real tools in this space

| # | Tool | What it is | Access model |
|---|------|-----------|--------------|
| 1 | **regexgen** (devongovett, npm + CLI) | Library/CLI that builds one regex matching an exact **set of literal strings** — trie → Hopcroft DFA minimisation → Brzozowski algebraic conversion, plus common-substring hoisting and character-class ranges. | Free, OSS, JS only |
| 2 | **Regex Generator** (olafneumann.org) | Browser tool: paste **one sample line**, click the parts you care about, and it assembles a regex from a library of recognised snippet types; shows the result plus language code snippets. | Free, web, OSS |
| 3 | **RegexMagic** (Just Great Software) | Commercial desktop generator: supply **matching samples and non-matching samples**, mark fields, pick a field/validation mode and capture groups, and it emits a pattern for ~14 flavours (JGsoft, .NET, Java, PCRE, JavaScript, Python, Ruby, POSIX BRE/ERE, XPath, …). | Paid desktop |

Others looked at and not ranked: `trie-regex` / `regex-trie` (thin subsets of regexgen), Regex
Generator++ (research, genetic-programming synthesis from labelled examples), AutoRegex-style
natural-language→regex sites (LLM-backed). The "generate strings **from** a regex" tools
(Browserling, Online String Tools, Regenerate) solve the inverse problem and are not competitors.

## Table stakes → our decision

| Capability | Seen in | In model? | Our decision |
|---|---|---|---|
| Multiple positive examples, one per line | 1, 3 | yes | `examples` (required, multiline textarea) + `separator` enum (`newline` default, `comma`, `tab`, `semicolon`, `space`) |
| Negative / non-matching examples | 3 | yes | `negatives` (optional) — every candidate pattern is **verified** against them |
| Anchor the whole string ("match whole line") | 2, 3 | yes | `anchors` boolean, default **true** (`^…$`) |
| Case-insensitive output | 1, 2, 3 | yes | `case_insensitive` boolean, default false |
| Generalise runs into character classes (`\d{4}`, `[A-Za-z]+`, ranges) | 2, 3 | yes | `generalize` strategy: per-position class merge with `{m,n}` quantifiers |
| Literal-set compression (trie / shared prefixes / `ba[rz]`) | 1 | yes | `alternation` strategy: prefix trie with common-prefix hoisting, sibling single chars folded to a class, empty branch → `?` |
| Exact vs loose quantifiers | 3 | yes | `quantifiers` enum `range` (default) / `exact` / `loose` |
| Capture groups around variable fields | 3 | yes | `capture_groups` boolean, default false |
| Multiple regex flavours | 2, 3 | partly | `flavor` enum `rust` (default), `pcre`, `python`, `javascript` (slash-literal with flags), `posix` (`[[:digit:]]`, no inline flags → cased literals expand to `[Aa]`) |
| Verification / highlighting against the samples | 3 | yes | Built in: candidates are compiled and run; a candidate is only accepted if it matches **every** positive and **no** negative. `json`/`report` output states exactly which leaked. |
| Explaining what the pattern means | 2 | yes | `output = report` renders a per-token plain-English breakdown |
| Presets for common shapes (dates, emails, IPs) | 2, 3 | yes (as page UX) | `[[example]]` chips on the page (dates, log levels, SKUs, order IDs, emails) instead of a preset param |
| 14 flavours incl. .NET/Java/Ruby/XPath/BRE | 3 | no | Out of model — untestable here; we ship the five we actually compile/verify against |
| Interactive click-the-sample-to-build UI | 2 | no | Out of model — needs a bespoke interactive canvas; our page is a form + deterministic inference |
| Genetic-programming / LLM synthesis from labelled examples | Regex Generator++, AutoRegex | no | Out of model — gizza is pure Rust/wasm with no model. We do **not** claim ML-grade synthesis; the tool documents its algorithm as deterministic class alignment + trie alternation |
| Host-language code snippets (Java/C#/Ruby boilerplate) | 2, 3 | no | Out of scope — flavour affects *pattern syntax only*, not surrounding code |
| Full Hopcroft DFA minimisation + Brzozowski conversion | 1 | rejected on judgment | Our trie factoring reaches the same shape on realistic inputs (`foo(?:ba[rz]|za p?)`-class output) at a fraction of the complexity; a full DFA minimiser is a large, hard-to-audit engine for marginal gain on human-sized example sets |

## Honesty constraints adopted

- The tool infers from **the examples it is given**; it is stated on the page and in the descriptor
  that this is deterministic structural inference, not learned/ML synthesis, and that a pattern that
  matches all samples can still be too tight or too loose for unseen data.
- Every emitted pattern is **verified before it is returned**. `auto` escalates
  `generalize → exact quantifiers → literal alternation` until the candidate matches all positives
  and rejects all negatives; if none can, the result says which negatives still match rather than
  silently shipping a wrong pattern.
- Caps are stated on the page: 200 000 characters of input, 5 000 examples, `max_alternatives`
  (default 50, max 500) distinct shapes/branches.
