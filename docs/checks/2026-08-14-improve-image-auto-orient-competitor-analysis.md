# image-auto-orient competitor analysis (2026-08-14)

Backlog item: `image-auto-orient` — auto-rotate a photo to its correct orientation using the EXIF orientation flag and bake it in.

## Search

Query: `online auto rotate photo EXIF orientation bake pixels tool orientation flag`

Reviewed reachable tools from the top results:

1. Elysia Tools — Rotate Image by EXIF
2. Pikdraw — Batch EXIF Auto-Rotate
3. ImageNurse — EXIF Orientation Fix
4. NoFileUpload — EXIF Orientation Fixer
5. AllOverTools — Image Rotator / EXIF Auto-Orient

This document paraphrases observed product capabilities only; no competitor copy, branding language, or examples are reused.

## Table-stakes capabilities

| Capability / UX pattern | Competitors | Fit | Decision in this block |
|---|---:|---|---|
| Read the file's EXIF Orientation tag and apply the indicated transform | all reviewed tools | in-model | Default `orientation=auto`; ffmpeg autorotation remains enabled for the input. |
| Bake the result into pixel data so tag-ignorant viewers display it upright | all reviewed tools | in-model | The output is a re-encoded image whose pixels are corrected. |
| Remove the orientation tag after correction to avoid later double-rotation | ImageNurse, NoFileUpload, Pikdraw emphasize this | in-model | ffmpeg output does not preserve the original EXIF orientation metadata; page copy documents this. |
| Handle all eight EXIF orientation values, including mirrored cases | dedicated EXIF fixers imply complete EXIF handling | in-model | Forced `orientation=1..8` enum maps all values to ffmpeg filters, including hflip/vflip/transpose variants. |
| Manual override when EXIF is missing or wrong | image rotator tools commonly expose manual rotation; fixers discuss no-tag cases | in-model | `orientation` enum allows `1` through `8` as explicit corrections. |
| Output format selection | image rotator/converter tools commonly expose output type | in-model | `format=same|jpeg|png|webp`. |
| JPEG/WebP quality setting | general image tools commonly expose quality | in-model | `quality` integer/slider 1-100; ignored for PNG. |
| Browser-local/private processing | several competitors advertise no upload | in-model for page | Page uses the existing browser ffmpeg runtime and describes local execution. CLI supports URL/ref via the existing gizza model. |
| Worked example for a sideways phone photo | most tools explain sideways phone photos | in-model | Page content includes an Orientation=6 example with dimension swap. |
| Batch upload / zip download | Pikdraw and some image tools support batches | out-of-model for this block | Gizza tool/page model takes one source file per run; documented as a limit. |
| Arbitrary-angle rotation/crop/editor controls | AllOverTools-style rotators | out-of-model | This backlog item is specifically EXIF auto-orientation, not an image editor; users should use a rotate/crop tool for arbitrary edits. |
| HEIC/HEIF phone photo support | some web services may accept it | out-of-model currently | The wasm ffmpeg/browser path does not reliably decode HEIC here; documented as unsupported. |
| Lossless JPEG transform without re-encoding | specialist desktop tools can do this | out-of-model currently | Current ffmpeg runtime path decodes/re-encodes; page documents re-encode and suggests PNG for lossless pixels. |

## Defaults and examples chosen

- Default correction: `auto`, because the main user problem is a phone photo that already carries an EXIF orientation tag.
- Default output format: `same`, to avoid surprising container changes.
- Default quality: `90`, matching common web image quality defaults while limiting generation loss.
- Preset chips:
  - Auto-fix a sideways photo (`orientation=auto`, `format=same`, `quality=90`)
  - Force 90° clockwise (`orientation=6`)
  - Upright PNG (`format=png`)
  - Un-mirror a selfie (`orientation=2`)

## Feasibility notes

The ffmpeg path was validated by building argv plans for every EXIF orientation value. `orientation=auto` relies on ffmpeg's default autorotation. Forced values disable autorotation with `-noautorotate` before `-i` and then apply an explicit `-vf` chain. This keeps the descriptor deterministic across CLI, chat block, and page wasm.
