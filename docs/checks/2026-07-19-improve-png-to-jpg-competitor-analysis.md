# png-to-jpg — competitor analysis (2026-07-19)

Scan done BEFORE implementing (create-next-tool step: competitor scan → descriptor design).
Paraphrased observations only — no competitor copy, branding, or trademarks reproduced.

## Competitors reviewed (top 3 reachable)

1. **ezgif PNG-to-JPG** — quality factor 0–100 (user-adjustable); background color to fill
   transparent areas via a color picker plus a manual hex field; default background white;
   upload by file dialog, drag-and-drop, or URL; 200 MB file cap; free, no watermark.
2. **png2jpg.com** — zero-configuration: transparency is auto-filled with white at a fixed
   "optimized" quality (no user controls); batch of up to 20 files; conversion runs fully
   in-browser via WebAssembly (privacy pitch: files never leave the device); warns that
   repeated JPG re-saves compound quality loss.
3. **FreeConvert PNG-to-JPG** — background color for transparent areas (picker + hex);
   optional resize of the output; compression options; auto-orient from EXIF; strip
   metadata toggle; save/apply presets; 1 GB cap (paid upgrades).

## Table stakes → decision (in-model / out-of-model)

| Capability | Seen at | Tag | Where it landed |
|---|---|---|---|
| Fill transparency with a chosen background color (picker + hex text, default **white**) | all 3 | in-model | `background` param, `kind = "color"` hybrid swatch+hex field, default `#ffffff`; accepts CSS names + `#RGB`/`#RRGGBB` via shared `normalize_ffmpeg_color` |
| JPEG quality control (0–100 slider) | ezgif, FreeConvert | in-model | `quality` integer 1–100 (family-consistent with image-convert/image-compress `quality_to_qv` mapping), default 85, `kind = "slider"` |
| In-browser / files never uploaded | png2jpg | in-model (platform) | ffmpeg.wasm page runs locally; stated generically in page copy |
| Drag-and-drop / paste upload | all 3 | in-model (platform) | ffmpeg pages get paste-to-upload + drop generically |
| One-click presets | FreeConvert | in-model | `[[example]]` chips: white bg, black bg, smaller-file quality 70 |
| URL input | ezgif | in-model (CLI/chat) | `url=` source on the CLI/chat surface |
| Batch conversion (up to 20 files) | png2jpg, FreeConvert | **out-of-model** | page file-input is single-upload; multi-input ffmpeg is un-buildable here (see page-patterns) — listed, not built |
| Resize during conversion | FreeConvert | **out-of-model** for this tool | covered by the sibling image-resize tool; keeping the converter focused |
| Strip metadata / auto-orient toggles | FreeConvert | **out-of-model** | ffmpeg re-encode already drops EXIF as a side effect; no separate toggle built |
| 200 MB–1 GB upload caps | ezgif, FreeConvert | n/a | page is local (browser-memory bound); chat/CLI fetch cap 8 MiB — stated on the page |

## Design decisions

- **ffmpeg type, not pure**, despite the backlog `pure` hint: the page (drag-drop convert) is
  the primary surface for this tool and the page file-input path is ffmpeg-runtime only —
  same shape as the sibling image-convert / image-compress / image-bg-replace.
- Flatten chain reuses the proven image-bg-replace solid-fill pattern:
  `split[a][b];[a]format=rgb24,drawbox=color=<C>:t=fill[bg];[bg][b]overlay` — the input is
  split, one copy flood-filled with the background color (alpha dropped via rgb24), and the
  original (with its alpha) is overlaid on top, so semi-transparent pixels blend correctly.
- `drawbox` (already proven on the page runtime by image-vignette) takes the
  `normalize_ffmpeg_color` token verbatim — names and hex both work, injection-safe charset.
- Quality default 85 matches image-convert's default and the common "web standard" guidance
  in the scan; mapping to `-q:v` mirrors `quality_to_qv` so the two tools agree.
- Differentiator vs the sibling image-convert: image-convert transcodes formats but has no
  background-color flatten (ffmpeg's default drops alpha onto black); png-to-jpg makes the
  fill color explicit and user-chosen — the exact gap the backlog row describes.

## Out-of-model list (not silently dropped)

- Batch/bulk multi-file conversion (single-upload page constraint).
- Resize-during-convert (sibling image-resize owns resizing).
- Metadata strip / EXIF auto-orient toggles (re-encode drops metadata anyway).
