# video-rounded-corners — competitor analysis (2026-08-22)

Scan run before implementation. Query: `round video corners online transparent mp4 webm`. I skimmed three reachable browser/video-editing tools and paraphrased behaviour only.

## Competitors skimmed

| # | Tool | URL | Shape |
|---|------|-----|-------|
| 1 | Kapwing rounded-corners video workflow | `https://www.kapwing.com/tools/round-corners` | Upload video/image, drag corner radius, export social-friendly video |
| 2 | Canva video frame/corner controls | `https://www.canva.com/features/video-editor/` | Design-editor frame masks with rounded cards and background colors |
| 3 | FFmpeg/geq/alpha community recipes | `https://stackoverflow.com/questions/65333678/ffmpeg-rounded-corners-with-alpha` | Command-line mask recipes for transparent rounded video |

## Table-stakes observed → decision

| # | Capability | Seen on | Decision | Where it lands |
|---|------------|---------|----------|----------------|
| 1 | Upload a video and export a rounded-corner clip | 1, 2 | IN | `Input::Video`, page file picker, CLI URL/ref source |
| 2 | Adjustable corner radius | 1, 2, 3 | IN | `radius` slider, 1-1000 px |
| 3 | Resolution-independent radius | 1, 2 | IN | `radius_unit=percent`, capped at 50% of shorter side |
| 4 | Transparent cut-off corners | 3 | IN | `background=transparent`, `format=webm` or `mov` |
| 5 | Solid background for ordinary MP4 exports | 1, 2 | IN | `background` color field; MP4 requires a non-transparent color |
| 6 | Output format choices | 1, 3 | IN | `format=webm|mp4|mov` |
| 7 | Round only selected corners | 2 | IN | `corners=all|top|bottom|left|right` |
| 8 | Keep/drop audio | 1 | IN | `keep_audio` checkbox |
| 9 | Quality/size control | 1 | IN | `quality` 1-100 mapped to codec CRF |
| 10 | Presets for common exports | 1, 2 | IN | `[[example]]` chips: transparent WebM, black MP4, top-only, MOV alpha |
| 11 | Live drag preview before encoding | 1, 2 | OUT | The current ffmpeg page model runs after file + param changes; no canvas-only preview layer |
| 12 | Arbitrary per-corner radius values | 2 | OUT | Current model has one radius plus selected side/corner groups; four independent radii would complicate UI and filter strings for little CLI value |
| 13 | Templates, stock media, timeline editing | 1, 2 | OUT | Design-app features, outside this pure ffmpeg block |

## Feasibility spike

A pure ffmpeg filter graph can build a rounded alpha mask with `format=rgba,geq=...`. Transparent output is viable for VP9/WebM and ProRes 4444/MOV. H.264/MP4 cannot carry alpha reliably, so the tool validates that MP4 uses a solid background and composites the rounded frame over a `drawbox` fill.

## UX controls adopted

- `radius` and `quality` use sliders.
- `radius_unit`, `corners`, and `format` are enums with labels.
- `background` uses the color control but still accepts `transparent` and named/hex colors.
- Preset chips mirror common export jobs.

## Stated limits

- 25 MiB input/output cap.
- Browser ffmpeg may be slow on long/high-resolution clips.
- Transparent corners require WebM or MOV; MP4 requires a solid background.
- Radius clamps to half the shorter side.
