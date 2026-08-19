# font-info competitor analysis — 2026-08-14

## Sources reviewed

Search query: `online font inspector TTF OTF WOFF font metadata glyph count metrics license tool`.

Representative tools from the result set:
- Font Analyzer Online Free / font-converters.com
- WhatFontIs Fonts Analyzer
- Made Good Designs Font Metadata Inspector
- Additional corroborating result: fontinfo.app / online font analyzer

This note paraphrases observed capabilities and uses them only to define table-stakes behavior for the gizza `font-info` block.

## Table-stakes capabilities

| Capability | Common competitor behavior | In model? | Decision for `font-info` |
| --- | --- | --- | --- |
| TTF / OTF / WOFF / WOFF2 input | Upload a desktop font or web font and inspect it locally/in-browser. | Yes | Auto-detect the container and decode WOFF/WOFF2 to SFNT before parsing. |
| Family/style names | Show family, subfamily, full name, PostScript name, version and preferred typographic names. | Yes | Report name-table fields under `names` with stable Windows/Unicode preference. |
| Foundry/license metadata | Surface copyright, trademark, manufacturer, designer, vendor/designer URLs, license text and license URL when present. | Yes | Include those name IDs and omit absent records rather than emitting nulls. |
| Glyph count | Display number of glyphs in the font. | Yes | Report `glyph_count` from the parsed face. |
| Metrics | Show units per em, ascender, descender, line gap, x-height/cap height and bounding box. | Yes | Report hhea/head metrics plus OS/2 typographic metrics when available. |
| Weight/width/style | Show CSS-ish weight, width, italic/bold/regular/monospace/variable classification. | Yes | Report OS/2 weight/width classes with friendly names and style flags. |
| Embedding permissions | Decode OS/2 fsType and explain installable/restricted/preview-print/editable permissions. | Yes | Report raw fsType, plain-English permission, subsetting and outline embedding flags. |
| Table list | Show the OpenType/SFNT table directory. | Yes | Report table count and tag/length entries from the decoded SFNT. |
| Unicode/cmap coverage | Some tools show character coverage or language support. | Partly | Count distinct Unicode code points and list cmap subtables; full language coverage maps are out of scope. |
| Variable axes | Font analyzers often identify variable font axes. | Yes | Report fvar axes when present. |
| Glyph outline preview | Some tools preview glyph paths, points and Bezier curves. | No | Out of model for this CLI/chat JSON report; no browser glyph viewer is built. |
| Shaping / ligature / kerning preview | Richer web tools allow typing sample text with OpenType features. | No | Out of model; requires shaping/preview UI rather than metadata inspection. |
| Full language support reports | Some tools summarize scripts/languages. | No | The tool reports raw Unicode mapping counts; script/language inference is not implemented. |

## UX / surface implications

`font-info` is a file-to-JSON inspection tool. Existing no-page file-input blocks in this repository expose chat and CLI surfaces rather than a generated pure-text page. The CLI should accept a `url=` or `ref=` source and produce deterministic JSON that can be exact-output tested. No presets, sliders or color controls apply because there are no tunable parameters: the font container is detected from bytes.

## Fit notes

A pure-Rust implementation fits the gizza model: `wuff` normalizes WOFF/WOFF2 to SFNT and `ttf-parser` reads OpenType metadata without native libraries or filesystem access. Font collections (`.ttc`/`.otc`) and legacy non-SFNT formats are rejected with explanatory errors because the selected parser accepts one face from an SFNT font, not collection selection UI.
