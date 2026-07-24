# html-entity-decoder — competitor analysis (2026-07-24)

Scan done BEFORE implementing. Search: "html entity decoder online". Skimmed the
top reachable tools; all copy/features below are **paraphrased** — no competitor
copy, branding, or trademarks reproduced.

## Competitors profiled

### C1 — emn178 online-tools "HTML Decode / Unescape"
- **Function:** decode named entities, decimal (`&#38;`) and hex (`&#x26;`)
  numeric references back to text.
- **Params/UX:** auto-update live preview as you type; input⇄output swap;
  copy-to-clipboard for result and share link; "remember input"; full-screen.
- **Copy angle:** security note — decoded text can become active markup if later
  rendered as HTML; sanitize untrusted input.

### C2 — onlinetexttools "HTML-decode Text"
- **Function:** decode both named and numeric (incl. hex) entities to plain text.
- **Params/UX:** left input / right output panes; file import; copy, download,
  save-as-file, export buttons; interactive examples; URL query-param support;
  error message on failed conversion.
- **Limits:** free tier is usage-capped / personal-use; commercial use gated.

### C3 — w3docs "HTML Encoder / Decoder"
- **Function:** four modes — encode-entities (decimal), decode-entities (decimal
  only, leaves hex + named untouched), encode-tags, decode-tags.
- **Params/UX:** source/result panes, reset + copy, fully client-side, single
  regex passes (no HTML parsing). Notably weaker: its decode only reverses
  `&#NN;` decimals and ignores named + hex refs.

(Also seen in results: elementor, binaryconvert, devtoolhub, teleport,
hidekazu-konishi — same decode-named+numeric core, some add a searchable entity
reference table of ~250 common entities.)

## Table-stakes → decisions

| Capability | In/out of model | Decision |
| --- | --- | --- |
| Decode **named** entities (full HTML5 set: `&mdash; &copy; &hellip; &trade; &nbsp;` …) | in-model | **Built** — backed by the `entities` crate's full WHATWG named-character-reference table (~2200 names). This is the core differentiator vs `string-escaper` (only 6 named entities). |
| Decode **decimal** `&#169;` and **hex** `&#xA9;`/`&#XA9;` numeric refs | in-model | **Built** — with WHATWG numeric handling. |
| WHATWG **C1 remap** (`&#151;` → em dash, `&#128;` → €) + invalid → U+FFFD | in-model | **Built** — Windows-1252 remap for 0x80–0x9F, replacement char for null/surrogate/out-of-range. Real Windows-authored HTML relies on this. |
| **Legacy** semicolon-less entities (`&copy` → ©, longest-prefix) | in-model | **Built** — the WHATWG legacy set decodes without `;`; non-legacy names still require `;`. |
| **Lenient** unknown handling (leave `&foo;` untouched) vs strict error | in-model | **Built** as `unknown` enum (`keep` default / `error`). Covers C2's "error on failed conversion" while keeping the standard lenient default. |
| Live preview / auto-run | in-model | Already provided — page auto-runs on input change. |
| Copy result / download output | in-model | Already provided — generator gives Copy + (text format) Download. |
| One-click **examples** | in-model | **Built** — `[[example]]` preset chips (named, numeric, legacy, mixed). |
| Security guidance copy | in-model (copy) | **Built** — FAQ + limits note that decoded output may be active HTML. |
| Encode direction / tag-encode | out-of-scope | This tool is decode-only per its spec; encoding is `string-escaper` (target=html, mode=escape). Noted, not built. |
| File import / save-as / share-link / accounts | out-of-model | Needs a backend/account or file pickers beyond the pure-text model; the page already accepts pasted text and offers copy/download. Considered, not built. |

## Fit note
gizza's existing `string-escaper` (target=html, mode=unescape) decodes only
`amp/lt/gt/quot/apos/nbsp` + numeric and **errors** on any other named entity
(e.g. `&mdash;`), so it does not cover this tool's own advertised examples. This
tool is the dedicated full-coverage decoder — a genuine capability gap, not a dup.
