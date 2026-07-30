# video-timelapse — competitor analysis (2026-07-30)

Snapshot for the new-tool + improve pass. Competitor notes are paraphrased only.
The in-model boundary is a local ffmpeg/WebAssembly tool that accepts one video
and returns one video; backend publishing, cloud storage, and AI interpolation are
out of scope.

## Competitor profiles

### 1. Kapwing timelapse / speed changer
- **Features:** speed up uploaded video, preview in a web editor, trim/crop, add
  audio or captions, export/share from a cloud workspace.
- **Params/options:** preset speed multipliers plus custom speed; output format
  handled by the editor; account/export flow for larger jobs.
- **UX:** drag-and-drop upload, timeline preview, template/editor controls.
- **Model fit:** speed multiplier is in-model; cloud editor/share/captioning are
  out-of-model.

### 2. Flixier video speed / timelapse editor
- **Features:** speed clips up on a timeline, combine multiple clips, add music,
  crop/resize, export in common video formats.
- **Params/options:** speed percentage/multiplier presets; timeline editing and
  cloud rendering.
- **UX:** multi-track editor, upload/import sources, preview before export.
- **Model fit:** the core speed-up is in-model; timeline editing and cloud render
  are not part of the single-tool runtime.

### 3. Clideo speed video
- **Features:** change playback speed from browser uploads/URLs/cloud drives,
  optionally mute audio, export a new video.
- **Params/options:** common speed presets (slow and fast); output container is
  handled automatically.
- **UX:** simple upload → choose speed → export flow.
- **Model fit:** speed preset and audio removal are in-model; cloud-drive import
  and server-side conversion are out-of-model.

### 4. VEED video speed controller
- **Features:** alter video speed, mute/keep audio depending on workflow, edit
  captions/text, export through a hosted editor.
- **Params/options:** preset and custom speed controls; extensive editor options.
- **UX:** drag/drop, timeline preview, branded editor surface.
- **Model fit:** speed control is in-model; editor/collaboration features are not.

### 5. Adobe Express / online video speed tool
- **Features:** preset video speed changes with guided upload/export and account
  integrations.
- **Params/options:** preset speed levels; no low-level fps/frame-drop control.
- **UX:** polished upload workflow and social export paths.
- **Model fit:** preset speed is in-model; account/template/social features are
  out-of-model.

## Table-stakes decisions

| Capability | Competitors | Our decision |
| --- | --- | --- |
| Speed multiplier | All tools expose presets/custom values | **Built** as `speed` 2–300×; examples cover 10×, 20×, and 60×. |
| Output frame rate | Usually hidden by editors | **Built** as `fps` 1–60 so users can choose 24/30/60 fps and control frame dropping explicitly. |
| Drop/mute audio | Common for timelapse workflows | **Built**: always `-an`; documented as deliberate because sped-up audio is usually unusable. |
| Browser-local processing | Some competitors process server-side | **Built** with ffmpeg/WASM page runtime; no upload. |
| H.264 shareable output | Table-stakes web compatibility | **Built** with `libx264`, `yuv420p`, `+faststart`; non-H.264-friendly containers switch to MP4. |
| Timeline editing / trim / captions / music | Cloud editors provide this | **Out-of-model** for a single block; adjacent gizza video tools cover trim/transcode/mute separately. |
| Motion interpolation / AI smoothing | Some desktop/editor products offer it | **Out-of-model**: no ML model; this tool drops frames rather than synthesizing frames. |
| Multi-file batch/export queues | Editor/backend products | **Out-of-model**: current gizza model is one input video → one output video. |

## Verification implications

The descriptor/page use numeric `speed` and `fps` controls with examples for
common presets. Tests assert both browser media output and the pure wasm ffmpeg
argv plan: `setpts=PTS/<speed>,fps=<fps>`, `-an`, H.264 encode, container
fallback from webm to mp4, and clamping at the advertised upper bounds.
