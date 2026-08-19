# video-grayscale — competitor analysis (2026-08-09)

Scan run **before** implementation so the descriptor could be designed to include the
in-model table stakes from the start. All notes are paraphrased observations of what each
competitor exposes to a user; no competitor copy, branding, or trademarks are reproduced or
imitated anywhere in this tool.

## Competitors reviewed

| # | Tool | Shape | What it exposes |
|---|------|-------|-----------------|
| 1 | mp4compress — "make video black & white" | server upload | MP4 in → MP4 out, 500 MB cap, zero settings (pick file → convert). Files deleted server-side after a few hours. |
| 2 | FileConverto — video grayscale | server upload | Single file picker (max 500 MB) + submit. No format, quality, or intensity controls. Markets the result as a vintage look. |
| 3 | Clideo — black & white video filter | server upload, account tiers | Filter *modes* (black & white is one of ~17, alongside sepia and other stylized looks), an **output-format selector** (MP4 recommended, several others offered), and cloud imports (Google Drive / Dropbox). Flow: upload → pick filter → pick format → process → download. |
| 4 | editclips.online — grayscale video | in-browser (closest analogue to ours) | **Mode**: grayscale / sepia / high-contrast B&W. **Output format**: MP4, MKV, MOV, WebM. **Quality tier**: quick / standard / best (speed-vs-size). Drag-drop + Ctrl+V paste, 100 MB cap, batch queue, before/after drag-to-compare preview, no watermark, states audio/resolution/frame-rate are preserved. |

Two of the four are single-button converters; the differentiation in this category comes from
the two that expose *how* the video is desaturated (mode/tint) and *how* it is re-encoded
(format/quality tier).

## Table stakes → in-model / out-of-model

| Capability | Seen at | Verdict | Where it landed |
|---|---|---|---|
| One-click grayscale of a common video, audio + resolution + fps untouched | all 4 | **in-model** | Defaults: `method=bt709`, `intensity=100`, `tint=none`, `contrast=1`, `keep_audio=true`. Resolution/fps are never touched. |
| Accepts the usual containers (MP4/MOV/MKV/WebM/AVI) | 1, 4 (Clideo implicit) | **in-model** | `Input::Video` / `accept="video/*"`. Container is kept when it can hold H.264+AAC; WebM (and anything else) comes out MP4 — documented on the page. |
| Stylized variants — sepia, other tones | 3, 4 | **in-model** | `tint` enum: `none` / `sepia` / `warm` / `cool` / `cyanotype`, folded into the same channel-mixer matrix (no extra pass). |
| "High-contrast B&W" mode | 4 | **in-model** | `contrast` (0.5–2.0, default 1) + a **High-contrast B&W** preset chip. Implemented as a trailing `eq=contrast=`. |
| Quality / speed tier | 4 | **in-model** | `quality` enum `fast` / `balanced` / `best` → x264 CRF 28 / 23 / 20. |
| Preview + download of the result | 3, 4 | **in-model** | The page's `format = "video"` renderer gives an inline `<video>` + download link. |
| Paste-to-upload (Ctrl+V) | 4 | **already in-model** | The ffmpeg page runtime provides paste-to-upload generically — nothing to build. |
| Stated size cap | 1, 2, 4 | **in-model** | 25 MB input cap, stated in the hero copy, the page notes, and the FAQ. |
| Presets / one-click looks | 3, 4 | **in-model** | Four `[[example]]` chips: Classic B&W, High-contrast B&W, Sepia, Faded 50%. |
| **Output-format selector** (MP4/MKV/MOV/WebM) | 3, 4 | **deliberately delegated, not dropped** | Re-encoding into a *different* container is `video-transcode`'s job (it already ships that enum). Duplicating it here would mean a second VP9/AV1 encoder path in this block for no new capability. The page says so explicitly and names the sibling tool. |
| Filter *intensity* / partial desaturation | none of the 4 | **in-model, ahead of the field** | `intensity` 0–100 slider — a true linear blend between the original and the toned gray, in one matrix. Competitors are all-or-nothing. |
| Luma weighting choice (BT.709/BT.601/average/channel mixer) | none of the 4 | **in-model, ahead of the field** | `method` enum. The R/G/B channel-filter options are the classic darkroom color-filter technique (a red filter darkens skies) and cost nothing extra — same single `colorchannelmixer`. |
| Drop the audio track (silent-film look) | none of the 4 | **in-model, ahead of the field** | `keep_audio` boolean (default on). |
| Cloud import (Google Drive / Dropbox) | 3 | **out-of-model** | This repo has no account/OAuth surface; input is a local file (page) or a public URL (CLI). |
| Batch queue / multiple files at once | 4 | **out-of-model** | The page file input is a single upload; multi-input is a known platform limitation. |
| ~17 stylized LUT filters (Moose/Reyes-style looks) | 3 | **out-of-model** | Stylized color grading is a different tool category, not grayscale conversion. |
| Before/after drag-to-compare slider | 4 | **out-of-model (page-shell scope)** | Would need a bespoke renderer in the shared page shell; the generic driver shows the source and the result. Listed, not built. |
| 100–500 MB caps | 1, 2, 4 | **out-of-model** | Those are server-side pipelines; ours is a wasm ffmpeg build in the user's tab, so the honest cap is 25 MB and it is stated up front rather than advertised and then failed. |

## Design decisions taken from the scan

1. **Default = one click.** Every param has a default, so the page runs correctly with nothing
   but a file chosen — matching the two zero-setting competitors. The extra knobs are opt-in.
2. **One filter pass.** `method`, `intensity` and `tint` all collapse into a single
   `colorchannelmixer` matrix (`out = (1-w)·original + w·tint·luma`), so adding a tone or a
   partial blend costs nothing at encode time. Only a non-1 `contrast` appends `eq=contrast=`.
3. **Presets as chips, not modes.** Where competitors ship named modes, the generator's
   `[[example]]` chips express the same thing declaratively while leaving every underlying
   value editable.
4. **Honest limits on the page.** Input cap, container-change rule (WebM → MP4), and the
   "grayscale barely shrinks the file" caveat (editclips makes the same point) are stated in
   the page copy and FAQ, not just in code.
5. **No copied copy.** Titles, hero text, FAQ wording, and preset names were written from
   scratch for this page.
