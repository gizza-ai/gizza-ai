# pdf-rotate — competitor analysis (2026-06-20)

Fourteenth `/create-next-tool` backlog pick. Pure-Rust (lopdf) Document tool;
chat + CLI (no page surface). Scoped to rotation; "reorders" (the row's second
verb) is deferred. Research via `WebSearch`, paraphrased.

## Competitors surveyed
| tool | does well (paraphrased) | dimension |
| ---- | ----------------------- | --------- |
| pdfresizer / Smallpdf / PDF2Go | right/left/180 + reset; rotate all or specific pages | capabilities |
| Lumin / Wondershare | per-page selection; save permanently | capabilities |
| pdfgear / Adobe | lossless rotation — sets the rotation flag, no re-encode | capabilities |

## Gap diff vs our tool
Our tool: add `degrees` (a multiple of 90; 90/180/270/-90) to each selected
page's `/Rotate`, normalized to 0/90/180/270; `pages` selects which (1-based list/
range or 'all'). This is exactly the **lossless** approach competitors highlight —
it sets the rotation flag without touching page content, and it's additive (stacks
on any existing rotation). It matches the full common feature set (direction +
selective pages + permanent + lossless).

**At parity — nothing to add this pass.** Notes:
- Direction is expressed as signed degrees (-90 = left, 90 = right, 180 = flip),
  which is more flexible than fixed buttons.
- Non-multiple-of-90 input is rejected (every competitor restricts to 90s).

**In-model gaps considered, deferred:**
- **Reorder pages** (the row's "or reorders") — rearrange the Pages /Kids order;
  a distinct operation, good as a follow-up (or a sibling pdf-reorder tool).
- **"odd"/"even" page selectors** — like pdf-split has; tiny convenience add.

**Out-of-model:** drag-thumbnail rotation UI (the page-form/chat model takes a
spec), auto-deskew (needs content analysis / a model).

## Tested
unit (6: rotate all, rotate selected only, additive+normalized (270+180→90),
negative normalized (-90→270), rejects non-multiple-of-90, rejects bad pdf/range)
+ drift-guard · `wafer build` validates the block · CLI on a real public 4-page
PDF (all pages get `/Rotate 90`, verified) + non-multiple-of-90 error path. No
page surface.

> Original work only — no competitor copy, branding, or trademarks copied.
