# sd-png-metadata-reader — competitor analysis (2026-07-22)

Scan of the top real tools that read Stable Diffusion generation metadata out of a PNG's text
chunks. All findings paraphrased — no competitor copy, branding, or trademarks reproduced.
gizza's `sd-png-metadata-reader` is a **no-page, image-input** tool (chat + CLI surfaces only,
like `image-metadata-viewer` / `image-info`): PNG bytes in via `url` **or** `ref`, structured
JSON report out.

## Competitors

### 1. SD Prompt Reader (receyuki/stable-diffusion-prompt-reader)
Open-source desktop viewer + CLI + `pip` package + ComfyUI node — the de-facto reference parser
the ecosystem copies. **Widest generator coverage:** A1111 WebUI, ComfyUI (full workflow JSON),
NovelAI (legacy + "stealth" alpha-channel pnginfo), InvokeAI (several schema versions), Draw
Things, Fooocus/Fooocus-MRE, StableSwarmUI, Easy Diffusion. Reads PNG text chunks (tEXt/iTXt;
zTXt auto-inflated) + EXIF for JPEG/WebP. **A1111 fields:** prompt, negative prompt, steps,
sampler, CFG scale, seed, size→w×h, model name, model hash, VAE, denoising strength, face
restoration, variation-seed strength; unknown trailing keys retained raw. **Missing-metadata:**
falls through detection chain, presents blank fields (no hard error). **UX:** drag-drop, copy,
export to `.txt`, edit/remove metadata, themes.

### 2. AI Image Metadata Viewer (Prompting Pixels)
Browser-based, fully client-side — the closest analog to a wasm tool. Generators: A1111/Forge,
ComfyUI, SwarmUI, Midjourney, Ideogram. Reads PNG tEXt + zTXt and EXIF; also video containers.
Parses the trailing settings line into key/value pairs (fields not individually enumerated).
**UX:** drag-drop, **batch**, per-datapoint copy, CSV + JSON export, all-local.

### 3. PNGchunk.com
Lightweight web viewer, SD-focused. Explicitly lists: prompt, negative prompt, sampler, seed,
CFG scale, steps, size, model hash, model. **UX:** file upload (≤25 MB) **or load-by-URL** (its
differentiator), RAW ↔ parsed toggle, single-file. Server-side processing.

## A1111 `parameters` format (the parse target)
Three regions in one multi-line string:
```
<positive prompt, may span lines>
Negative prompt: <negative prompt, may span lines>
Steps: 35, Sampler: DPM2, Schedule type: Karras, CFG scale: 7, Seed: 140850743528, Size: 960x640, Model hash: 81d4d52035, Model: foo, Denoising strength: 0.4, Clip skip: 2, Lora hashes: "myLora: abc123", Version: v1.10.1
```
Parse: text before `Negative prompt:` = positive; between it and the settings line = negative;
the last line is the comma-separated `Key: value` settings blob. Edge cases: quoted values that
contain commas (`Lora hashes`, `TI hashes`) must not be split on their inner commas; `Size` is
split on `x`. Common keys: Steps, Sampler, Schedule type, CFG scale, Seed, Size, Model hash,
Model, VAE, VAE hash, Denoising strength, Clip skip, Hires upscale/steps/upscaler, Lora hashes,
TI hashes, Version, plus extension keys (ControlNet, ADetailer, …).

## Table-stakes → where each lands

| Capability | Status in our tool |
|---|---|
| Split positive / negative / settings | **Built** — `parse_a1111`, receyuki's last-line algorithm |
| Per-field values (steps, sampler, cfg, seed, size→w×h, model, model hash) | **Built** — typed convenience fields |
| Every settings `Key: value` pair retained | **Built** — `params` map (keeps unknown keys: Schedule type, VAE, Clip skip, Denoising, Hires*, Lora hashes, Version, ControlNet…) |
| Quoted-value comma edge case | **Built** — quote-aware comma splitter + `\`-unescape |
| `Size` → width/height | **Built** — `width`/`height` fields |
| RAW view of the chunk text | **Built** — `raw_chunks[]` returns every tEXt/iTXt/zTXt keyword+text verbatim, alongside the parsed fields (the report IS both raw and parsed) |
| tEXt / iTXt / **zTXt (inflate)** | **Built** — byte-level chunk walk + `flate2` zlib inflate; iTXt UTF-8, compressed-iTXt inflate |
| Generator detection | **Built** — A1111 / ComfyUI / NovelAI / InvokeAI heuristics (`generator` field) |
| Graceful "no SD metadata" | **Built** — `has_sd_metadata:false` with all chunks still returned; non-PNG → clear error |
| Load-by-URL | **Built (in-model here)** — the block accepts `url`; `resolve_source` fetches it server-side (SSRF-guarded), so URL input works without a CORS proxy |

## Considered, not built (out-of-model or out-of-surface)
- **Model-hash → model-name DB lookup** (Civitai): needs network + a hosted model DB — out-of-model.
- **NovelAI "stealth" alpha-channel pnginfo:** requires decoding the PNG pixel buffer and reading
  LSBs of the alpha channel; feasible in pure Rust but out of scope for a text-chunk reader
  (deferred — the standard NovelAI `Comment`/`Software` chunks ARE detected).
- **ComfyUI/InvokeAI workflow JSON → structured node graph:** we return the raw JSON chunk text
  (and detect the generator) but don't walk the node graph into typed fields — the graph shape is
  version-specific; raw text is the honest, stable surface.
- **JPEG/WebP EXIF `parameters`:** this tool is PNG-chunk-scoped (the description says "PNG's text
  chunks"); EXIF-embedded SD params on JPEG/WebP are covered conceptually by `image-metadata-viewer`.
- **Batch / CSV export / drag-drop / copy buttons / RAW toggle UI:** page-UX features. This is a
  no-page tool (image input + text report), so there's no page surface; the JSON already carries
  both raw and parsed data for a caller to format.

## Not a duplicate
`image-metadata-viewer` reads **EXIF/TIFF** (camera/GPS) via kamadak-exif; `image-info` reports
format/dimensions; `png-chunk-stripper` *removes* chunks. None parse PNG **text** chunks or the
A1111 `parameters` string. This tool is distinct.
