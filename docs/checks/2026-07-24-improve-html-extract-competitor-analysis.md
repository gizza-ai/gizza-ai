# html-extract — competitor analysis (2026-07-24)

Paraphrased only. No competitor copy, branding, or trademarks reproduced.

**Tool function:** paste raw HTML, run a CSS selector over it, and pull out the
text / inner HTML / outer HTML / a named attribute from every matching element —
"jq for markup". Pure, in-browser; NOT a URL fetcher (that surface already exists
as `blocks/css-select-extract`, which fetches over the network). This tool is the
paste-in, page-able sibling.

## Competitors skimmed (top 3 reachable)

1. **Scrapfly CSS Selector & XPath Tester** — pasted-HTML editor (syntax
   highlighting) + selector field. Extraction modes: full element HTML, text
   content, attribute values. Live match count, per-match display truncated at
   ~200 chars, copy-one / copy-all, download results, "no matches" feedback,
   invalid-selector error message, preset template snippets (e-commerce, blog,
   table, nested).
2. **Agenty CSS Extractor** — five extract types: TEXT (tags stripped), HTML
   (outer, includes the parent element's own tags), ATTR (needs an attribute
   name, e.g. `href`), InnerHTML (children only, no parent tags), OuterText (text
   at the element level excluding nested children). ATTR mode requires the user to
   name the attribute.
3. **Aspose CSS Selector** — URL-based (not paste); selector field + Find;
   returns the matching elements. Confirms the URL-fetch surface is a separate
   product from a paste-in extractor.

(OpenGraph.io Web Extract and Elysiatools also seen in results — URL-based
structured extractors, same URL-vs-paste split.)

## Table-stakes → decision

| Capability | Competitors | In/out-of-model | Decision |
|---|---|---|---|
| Paste HTML input | Scrapfly, Agenty | in-model (pure text field) | **built** — `html` param, multiline |
| CSS selector field | all | in-model | **built** — `selector`, required |
| Extract = text | all | in-model (`scraper` `.text()`) | **built** — `extract=text` (default) |
| Extract = inner HTML | Agenty, Scrapfly | in-model (`.inner_html()`) | **built** — `extract=inner-html` |
| Extract = outer/full-element HTML | Agenty (HTML), Scrapfly (full element) | in-model (`.html()`) | **built** — `extract=outer-html` |
| Extract = attribute value | all | in-model (`.attr(name)`) | **built** — `extract=attr` + `attr` name |
| Attribute-name input for ATTR | Agenty | in-model | **built** — `attr` param, validated required for attr mode |
| Match count | Scrapfly | in-model | **built** — `count` in chat/CLI JSON |
| Limit / cap results | (implicit; Scrapfly truncates display) | in-model | **built** — `limit` (default 100, min 1) |
| Whitespace trim / normalize | implied by "text (tags stripped)" cleanliness | in-model | **built** — `trim` boolean (default on): normalizes text/attr whitespace, trims html ends |
| OuterText (element-level text only) | Agenty | in-model but niche/ambiguous | **not built** — rarely needed; `text` + a tighter selector covers it. Listed, not built. |
| XPath selectors | Scrapfly | out-of-model here (`scraper` is CSS-only; no wasm-safe XPath engine proven) | **out-of-model** — CSS only, stated on page |
| Live preview / match highlighting on rendered DOM | Scrapfly | out-of-model (needs a live DOM render; page is recompute-on-input text) | **out-of-model** — recompute-on-change instead |
| Generated code snippets (Python/JS) | Scrapfly | out-of-scope for a gizza tool page | **out-of-model** — generator emits a CLI example instead |
| Preset template chips | Scrapfly | in-model (generator `[[example]]` chips) | **built** — example chips prefill selector + HTML |

## UX patterns adopted
- Multiline `html` textarea (preserves pasted newlines).
- `extract` as a `<select>` (`Param::enumv`) with friendly labels via `[input.labels]`.
- `trim` checkbox default-on (raw `scraper` text is heavily indented; normalizing
  is the sane default).
- `[[example]]` preset chips for one-click worked examples (links `href`, headings text).
