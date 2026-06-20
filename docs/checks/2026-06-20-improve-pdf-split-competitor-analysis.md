# pdf-split — competitor analysis (2026-06-20)

Third `/create-next-tool` backlog pick. Document-input tool (PDF in → PDF out of
selected pages), chat + CLI only — no page (can't fetch an arbitrary PDF).
Research via `WebSearch`. All findings **paraphrased**.

## Competitors surveyed
| tool | does well (paraphrased) | dimension |
| ---- | ----------------------- | --------- |
| iLovePDF / Adobe / Xodo | custom page ranges; split by N pages or by bookmarks; client-side | capabilities |
| pdfresizer / pdfchef | from/to range, extract specific pages, odd/even selection | capabilities |
| splitapdf / freepdfconvert | extract pages into a new PDF; ZIP of individual pages; batch multiple files | capabilities / UX |

## Gap diff vs our tool
Our tool: `pages` spec = 1-based comma list + inclusive ranges, plus `all`/`odd`/
`even`; deletes the unselected pages, prunes orphans, renumbers, re-serializes;
output order follows the source.

**In-model gap closed in this pass:**
- **odd / even page selection** — common splitter convenience; added as `pages`
  keywords with tests + CLI verification.

**In-model gaps considered, deferred (fit the model; good follow-ups):**
- **Split-by-N** (every N pages → multiple PDFs) and **per-page ZIP** — our
  envelope returns a single artifact; multi-file output needs an envelope that
  carries several files (a cross-tool change), so it's out of scope for one tool.
- **Split by top-level bookmarks** — needs outline parsing; a focused follow-up.

**Out-of-model:** batch across many uploaded files at once (no multi-file upload
surface in chat), accounts, server tiers.

## Tested
unit (8: list/range/odd/even/reversed parse, out-of-range + zero + non-numeric
rejects, keep-selected, range-count, all, out-of-range split, non-pdf) +
drift-guard · `wafer build` validates the block · CLI end-to-end on a real
4-page public PDF (page-1 96KB < all 144KB; odd vs even produce distinct valid
PDFs) + error paths (out-of-range, bad spec, wrong content-type). No page surface.

> Original work only — no competitor copy, branding, or trademarks copied.
