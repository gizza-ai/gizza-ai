# pdf-resize-pages — competitor analysis (2026-08-22)

Scan run before shipping the tool. Notes are behavioural observations only; no competitor copy or branding is reused.

## Scope

Backlog row: "Changes page size (e.g., A4 to US Letter) and scales content to fit." Type hint `pure`.

## Duplicate check

- Existing PDF tools cover split/merge/extract/metadata/encrypt/decrypt/text/table/OCR-adjacent tasks, but none rewrite page boxes and scale the existing page artwork to a new paper size.
- This is distinct from image/PDF table tools: the output remains a vector PDF and does not rasterise pages.

## Reference tools reviewed

1. Adobe Acrobat page-size/preflight workflows — common target sizes, orientation, scale-to-fit expectations.
2. Sejda/online PDF resize pages tools — preset page sizes plus custom dimensions, scale/fit controls, selected pages.
3. PDF24/online page resizing tools — A-series/Letter/Legal presets, margins, and no-upload-local/processed-output download UX patterns.

## Table stakes and decisions

| Capability | Decision |
| --- | --- |
| Standard target sizes (A-series, Letter, Legal, Tabloid, Executive, Statement) | In-model: `size` enum. |
| Custom width/height with common units | In-model: `size=custom`, `width`, `height`, `unit` (`mm`, `cm`, `in`, `pt`). |
| Keep or force orientation | In-model: `orientation=auto|portrait|landscape`. |
| Fit content rather than clipping | In-model default: `scale=fit`. |
| Alternative scaling semantics | In-model: `fill`, `stretch`, `none`, plus `zoom` percent. |
| Margins around fitted content | In-model: `margin` in the selected unit. |
| Resize selected pages only | In-model: `pages=all|odd|even|1,3-5`. |
| Keep text/vector quality | In-model: implemented with PDF transform matrices; no rasterisation. |
| Preserve annotations/links visually | In-model: annotation rectangles are transformed with the page content. |
| Browser drag-and-drop page | Out-of-scope for this block: binary PDF source/output tools in this repo are chat/CLI surfaces; no page was added. |
| Password-protected PDFs, form-field appearance regeneration, imposition/n-up | Out-of-model for this pass; listed rather than partially built. |

## Verification design

The unit suite creates tiny PDFs with `lopdf`, resizes them, reparses the output, and checks page boxes, selected-page behaviour, scale modes, margins, rotation, annotations, stale box removal, and error paths. CLI verification uses a generated local fixture served over HTTP because the CLI source resolver accepts public HTTP(S)/ref inputs rather than local file paths.
